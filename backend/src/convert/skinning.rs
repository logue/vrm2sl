use std::collections::{HashMap, HashSet};

use anyhow::Result;
use serde_json::Value;

use super::gltf_utils::{
    PrimitiveSkinBinding, accessor_meta, read_joint_slot, read_weight_f32, write_joint_slot,
    write_weight_f32,
};
use super::types::{BENTO_BONE_MAP, BONE_MAP};

// ─── Unmapped-bone weight remapping ──────────────────────────────────────────

/// Remap vertex weights from non-SL (unmapped) VRM bones to their nearest
/// SL-mapped ancestor in the skeleton hierarchy.
///
/// Unmapped bones include:
/// - `upperChest` (not in BONE_MAP; SL uses only chest/spine/neck chain)
/// - Spring/secondary bones (`J_Sec_*`) used for clothing/hair physics in VRM
///
/// For each skin, joint-slot indices that refer to unmapped nodes have their
/// accumulated weight transferred to the nearest ancestor that IS mapped.
pub(super) fn remap_unmapped_bone_weights(
    json: &mut Value,
    bin: &mut [u8],
    humanoid_bone_nodes: &HashMap<String, usize>,
) {
    let sl_node_indices: HashSet<usize> = BONE_MAP
        .iter()
        .chain(BENTO_BONE_MAP.iter())
        .filter_map(|(vrm_name, _)| humanoid_bone_nodes.get(*vrm_name).copied())
        .collect();

    let node_count = json["nodes"].as_array().map(|a| a.len()).unwrap_or(0);
    let mut parent_of = vec![None::<usize>; node_count];
    if let Some(nodes) = json["nodes"].as_array() {
        for (parent_idx, node) in nodes.iter().enumerate() {
            if let Some(children) = node["children"].as_array() {
                for child in children {
                    if let Some(child_idx) = child.as_u64().map(|v| v as usize) {
                        if child_idx < parent_of.len() {
                            parent_of[child_idx] = Some(parent_idx);
                        }
                    }
                }
            }
        }
    }

    let find_sl_ancestor = |start: usize| -> Option<usize> {
        let mut cur = parent_of[start];
        while let Some(p) = cur {
            if sl_node_indices.contains(&p) {
                return Some(p);
            }
            cur = parent_of.get(p).and_then(|v| *v);
        }
        None
    };

    let skin_count = json["skins"].as_array().map(|s| s.len()).unwrap_or(0);

    for skin_index in 0..skin_count {
        let joints: Vec<usize> = json["skins"][skin_index]["joints"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();

        if joints.is_empty() {
            continue;
        }

        let mut slot_remap: Vec<usize> = (0..joints.len()).collect();
        let mut any_remap = false;
        for (slot, &node_idx) in joints.iter().enumerate() {
            if sl_node_indices.contains(&node_idx) {
                continue;
            }
            if let Some(ancestor_node_idx) = find_sl_ancestor(node_idx) {
                if let Some(ancestor_slot) = joints.iter().position(|&j| j == ancestor_node_idx) {
                    slot_remap[slot] = ancestor_slot;
                    any_remap = true;
                }
            }
        }

        if !any_remap {
            continue;
        }

        let bindings = collect_skin_primitive_bindings(json, skin_index);
        for binding in bindings {
            let Some(joints_meta) = accessor_meta(json, binding.joints_accessor) else {
                continue;
            };
            let Some(weights_meta) = accessor_meta(json, binding.weights_accessor) else {
                continue;
            };
            if joints_meta.accessor_type != "VEC4" || weights_meta.accessor_type != "VEC4" {
                continue;
            }
            if !(joints_meta.component_type == 5121 || joints_meta.component_type == 5123) {
                continue;
            }
            if weights_meta.component_type != 5126 {
                continue;
            }

            let count = joints_meta.count.min(weights_meta.count);
            for vertex_index in 0..count {
                let mut slots = [0u16; 4];
                let mut weights = [0.0f32; 4];
                for lane in 0..4 {
                    slots[lane] =
                        read_joint_slot(bin, &joints_meta, vertex_index, lane).unwrap_or(0);
                    weights[lane] =
                        read_weight_f32(bin, &weights_meta, vertex_index, lane).unwrap_or(0.0);
                }

                let mut acc = vec![0.0f32; joints.len()];
                for lane in 0..4 {
                    let old_slot = slots[lane] as usize;
                    if old_slot >= slot_remap.len() {
                        continue;
                    }
                    let target_slot = slot_remap[old_slot];
                    if target_slot < acc.len() {
                        acc[target_slot] += weights[lane];
                    }
                }

                let mut top4: Vec<(usize, f32)> = acc
                    .iter()
                    .enumerate()
                    .filter(|&(_, &w)| w > 1e-7)
                    .map(|(s, &w)| (s, w))
                    .collect();
                top4.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                top4.truncate(4);

                let weight_sum: f32 = top4.iter().map(|&(_, w)| w).sum();
                let mut new_slots = [0u16; 4];
                let mut new_weights = [0.0f32; 4];
                for lane in 0..4 {
                    if let Some(&(slot, w)) = top4.get(lane) {
                        new_slots[lane] = slot as u16;
                        new_weights[lane] = if weight_sum > 1e-7 {
                            w / weight_sum
                        } else {
                            0.0
                        };
                    }
                }

                for lane in 0..4 {
                    let _ =
                        write_joint_slot(bin, &joints_meta, vertex_index, lane, new_slots[lane]);
                    let _ =
                        write_weight_f32(bin, &weights_meta, vertex_index, lane, new_weights[lane]);
                }
            }
        }
    }
}

// ─── Skinning weight optimization ────────────────────────────────────────────

/// Remove unused joint slots from all skins, compacting the joints list and
/// inverse bind matrix accessor in-place.
pub(super) fn optimize_skinning_weights_and_joints(json: &mut Value, bin: &mut [u8]) -> Result<()> {
    let skin_count = json
        .get("skins")
        .and_then(Value::as_array)
        .map(|skins| skins.len())
        .unwrap_or(0);

    for skin_index in 0..skin_count {
        let bindings = collect_skin_primitive_bindings(json, skin_index);
        if bindings.is_empty() {
            continue;
        }

        let joints_len = json
            .get("skins")
            .and_then(Value::as_array)
            .and_then(|skins| skins.get(skin_index))
            .and_then(|skin| skin.get("joints"))
            .and_then(Value::as_array)
            .map(|joints| joints.len())
            .unwrap_or(0);

        if joints_len == 0 {
            continue;
        }

        let mut used_slots = vec![false; joints_len];
        for binding in &bindings {
            scan_used_joint_slots(json, bin, *binding, &mut used_slots);
        }

        let mut keep_slots: Vec<usize> = used_slots
            .iter()
            .enumerate()
            .filter_map(|(index, used)| if *used { Some(index) } else { None })
            .collect();
        if keep_slots.is_empty() {
            keep_slots = (0..joints_len).collect();
        }

        let mut old_to_new = vec![None; joints_len];
        for (new_index, old_index) in keep_slots.iter().copied().enumerate() {
            old_to_new[old_index] = Some(new_index as u16);
        }

        for binding in &bindings {
            remap_primitive_joints_and_weights(json, bin, *binding, &old_to_new);
        }

        compact_skin_joints_and_inverse_bind_matrices(json, bin, skin_index, &keep_slots)?;
    }

    Ok(())
}

/// Stabilize tiny head/eye-only secondary skins by forcing a single `mHead`
/// bind inside each skin.
///
/// This avoids importer instability with tiny facial skins that include eye
/// joints while keeping skin-index topology unchanged.
pub(super) fn collapse_secondary_head_skins_to_primary(
    json: &mut Value,
    bin: &mut [u8],
    humanoid_bone_nodes: &HashMap<String, usize>,
) {
    let Some(skins) = json.get("skins").and_then(Value::as_array) else {
        return;
    };
    if skins.len() <= 1 {
        return;
    }

    let primary_skin_index = skins
        .iter()
        .enumerate()
        .max_by_key(|(_, skin)| {
            skin.get("joints")
                .and_then(Value::as_array)
                .map(|j| j.len())
                .unwrap_or(0)
        })
        .map(|(i, _)| i);
    let Some(primary_skin_index) = primary_skin_index else {
        return;
    };

    let Some(head_node_index) = humanoid_bone_nodes.get("head").copied() else {
        return;
    };

    let mut used_skin_indices = HashSet::<usize>::new();
    if let Some(nodes) = json.get("nodes").and_then(Value::as_array) {
        for node in nodes {
            if let Some(si) = node.get("skin").and_then(Value::as_u64).map(|v| v as usize) {
                used_skin_indices.insert(si);
            }
        }
    }

    for skin_index in 0..skins.len() {
        if skin_index == primary_skin_index || !used_skin_indices.contains(&skin_index) {
            continue;
        }

        let bound_node_names: Vec<String> = json
            .get("nodes")
            .and_then(Value::as_array)
            .map(|nodes| {
                nodes
                    .iter()
                    .filter_map(|node| {
                        let node_skin =
                            node.get("skin").and_then(Value::as_u64).map(|v| v as usize);
                        if node_skin != Some(skin_index) {
                            return None;
                        }
                        Some(
                            node.get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                        )
                    })
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        let is_hair_skin = !bound_node_names.is_empty()
            && bound_node_names
                .iter()
                .all(|name| name.to_ascii_lowercase().contains("hair"));
        if !is_hair_skin {
            continue;
        }

        let joints: Vec<usize> = json["skins"][skin_index]["joints"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();
        if joints.is_empty() {
            continue;
        }

        // Limit to pure head-only tiny skins so body limb skins are untouched.
        // Keep eye-containing face skins intact; collapsing them to mHead can
        // hide/misplace iris geometry in preview and import.
        let is_tiny_head_skin = joints.len() <= 2
            && joints.iter().all(|&node_idx| {
                let name = json["nodes"][node_idx]
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                matches!(name, "mHead")
            });
        if !is_tiny_head_skin {
            continue;
        }

        let bindings = collect_skin_primitive_bindings(json, skin_index);
        for binding in bindings {
            let Some(joints_meta) = accessor_meta(json, binding.joints_accessor) else {
                continue;
            };
            let Some(weights_meta) = accessor_meta(json, binding.weights_accessor) else {
                continue;
            };
            if joints_meta.accessor_type != "VEC4" || weights_meta.accessor_type != "VEC4" {
                continue;
            }
            if !(joints_meta.component_type == 5121 || joints_meta.component_type == 5123) {
                continue;
            }
            if weights_meta.component_type != 5126 {
                continue;
            }

            let count = joints_meta.count.min(weights_meta.count);
            for vertex_index in 0..count {
                let _ = write_joint_slot(bin, &joints_meta, vertex_index, 0, 0);
                let _ = write_joint_slot(bin, &joints_meta, vertex_index, 1, 0);
                let _ = write_joint_slot(bin, &joints_meta, vertex_index, 2, 0);
                let _ = write_joint_slot(bin, &joints_meta, vertex_index, 3, 0);

                let _ = write_weight_f32(bin, &weights_meta, vertex_index, 0, 1.0);
                let _ = write_weight_f32(bin, &weights_meta, vertex_index, 1, 0.0);
                let _ = write_weight_f32(bin, &weights_meta, vertex_index, 2, 0.0);
                let _ = write_weight_f32(bin, &weights_meta, vertex_index, 3, 0.0);
            }
        }

        // Rewrite this skin to a single-joint mHead bind.
        if let Some(skins_mut) = json.get_mut("skins").and_then(Value::as_array_mut)
            && let Some(skin_mut) = skins_mut.get_mut(skin_index)
        {
            skin_mut["joints"] = Value::Array(vec![Value::from(head_node_index as u64)]);

            if let Some(acc_idx) = skin_mut
                .get("inverseBindMatrices")
                .and_then(Value::as_u64)
                .map(|v| v as usize)
                && let Some(accessors) = json.get_mut("accessors").and_then(Value::as_array_mut)
                && let Some(accessor) = accessors.get_mut(acc_idx)
            {
                accessor["count"] = Value::from(1u64);
            }
        }
    }
}

/// Force face skin to a single `mHead` bind to avoid crossed-eye deformation in
/// importers with different eye-bone rest handling.
///
/// Also compacts `skin.joints` to `[mHead]` and resets every vertex's joint
/// reference to slot 0, so that no ghost eye-joints with non-identity IBM
/// rotations remain in the skin at upload time.  The IBM is subsequently
/// regenerated by `regenerate_inverse_bind_matrices`.
pub(super) fn soften_face_eye_influences(json: &mut Value, bin: &mut [u8]) {
    let skin_count = json
        .get("skins")
        .and_then(Value::as_array)
        .map(|s| s.len())
        .unwrap_or(0);

    for skin_index in 0..skin_count {
        let bound_node_names: Vec<String> = json
            .get("nodes")
            .and_then(Value::as_array)
            .map(|nodes| {
                nodes
                    .iter()
                    .filter_map(|node| {
                        let node_skin =
                            node.get("skin").and_then(Value::as_u64).map(|v| v as usize);
                        if node_skin != Some(skin_index) {
                            return None;
                        }
                        Some(
                            node.get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                        )
                    })
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        let is_face_skin = bound_node_names
            .iter()
            .any(|name| name.to_ascii_lowercase().contains("face"));
        if !is_face_skin {
            continue;
        }

        let joints: Vec<usize> = json["skins"][skin_index]["joints"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();
        if joints.is_empty() {
            continue;
        }

        let mut head_slot = None;
        for (slot, node_idx) in joints.iter().copied().enumerate() {
            let name = json["nodes"][node_idx]
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("");
            if name == "mHead" {
                head_slot = Some(slot);
            }
        }
        let Some(head_slot) = head_slot else {
            continue;
        };

        // The node index of mHead (needed to rewrite skin.joints below).
        let head_node_index = joints[head_slot];

        let bindings = collect_skin_primitive_bindings(json, skin_index);
        for binding in bindings {
            let Some(joints_meta) = accessor_meta(json, binding.joints_accessor) else {
                continue;
            };
            let Some(weights_meta) = accessor_meta(json, binding.weights_accessor) else {
                continue;
            };
            if joints_meta.accessor_type != "VEC4" || weights_meta.accessor_type != "VEC4" {
                continue;
            }
            if !(joints_meta.component_type == 5121 || joints_meta.component_type == 5123) {
                continue;
            }
            if weights_meta.component_type != 5126 {
                continue;
            }

            let count = joints_meta.count.min(weights_meta.count);
            for vertex_index in 0..count {
                // Write slot 0 (not head_slot) so that the vertex references
                // the first (and only) joint after we compact to [mHead].
                let _ = write_joint_slot(bin, &joints_meta, vertex_index, 0, 0);
                let _ = write_joint_slot(bin, &joints_meta, vertex_index, 1, 0);
                let _ = write_joint_slot(bin, &joints_meta, vertex_index, 2, 0);
                let _ = write_joint_slot(bin, &joints_meta, vertex_index, 3, 0);

                let _ = write_weight_f32(bin, &weights_meta, vertex_index, 0, 1.0);
                let _ = write_weight_f32(bin, &weights_meta, vertex_index, 1, 0.0);
                let _ = write_weight_f32(bin, &weights_meta, vertex_index, 2, 0.0);
                let _ = write_weight_f32(bin, &weights_meta, vertex_index, 3, 0.0);
            }
        }

        // If mHead is not already the first joint, copy its IBM to slot 0 so
        // that after compacting joints=[mHead], slot 0 holds the correct matrix.
        // Without this, IBM[0] stays as the first original joint's matrix
        // (often mEyeLeft) which gives the wrong joint position override in SL.
        if head_slot != 0 {
            let ibm_acc_idx = json["skins"][skin_index]
                .get("inverseBindMatrices")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            if let Some(ibm_idx) = ibm_acc_idx {
                if let Some(meta) = accessor_meta(json, ibm_idx) {
                    // MAT4 = 16 f32 = 64 bytes; meta.stride encodes the actual
                    // bytes-per-element including any bufferView byteStride.
                    let bytes_per_matrix = meta.stride; // typically 64
                    let src_start = meta.base_offset + head_slot * bytes_per_matrix;
                    let dst_start = meta.base_offset;
                    if src_start + bytes_per_matrix <= bin.len()
                        && dst_start + bytes_per_matrix <= bin.len()
                        && src_start != dst_start
                    {
                        bin.copy_within(src_start..src_start + bytes_per_matrix, dst_start);
                    }
                }
            }
        }

        // Compact skin.joints to [mHead only] and update IBM accessor count +
        // bufferView byteLength so importers that trust byteLength over count
        // don't read stale IBM data from the eye-joint slots.
        // Ghost eye joints with non-identity IBM rotations would otherwise
        // appear in the SL uploader as spurious position overrides.
        let ibm_acc_idx = json["skins"][skin_index]
            .get("inverseBindMatrices")
            .and_then(Value::as_u64)
            .map(|v| v as usize);

        if let Some(skins_mut) = json.get_mut("skins").and_then(Value::as_array_mut)
            && let Some(skin_mut) = skins_mut.get_mut(skin_index)
        {
            skin_mut["joints"] = Value::Array(vec![Value::from(head_node_index as u64)]);
        }

        if let Some(acc_idx) = ibm_acc_idx {
            // Read bufferView index before taking mutable borrows.
            let bv_idx = json
                .get("accessors")
                .and_then(Value::as_array)
                .and_then(|accs| accs.get(acc_idx))
                .and_then(|acc| acc.get("bufferView"))
                .and_then(Value::as_u64)
                .map(|v| v as usize);

            if let Some(accs) = json.get_mut("accessors").and_then(Value::as_array_mut)
                && let Some(acc) = accs.get_mut(acc_idx)
            {
                acc["count"] = Value::from(1u64);
            }

            // Trim bufferView byteLength to exactly 1 MAT4 (64 bytes).
            if let Some(bv_index) = bv_idx
                && let Some(bv_arr) = json.get_mut("bufferViews").and_then(Value::as_array_mut)
                && let Some(bv) = bv_arr.get_mut(bv_index)
            {
                let current = bv.get("byteLength").and_then(Value::as_u64).unwrap_or(0) as usize;
                if current > 64 {
                    bv["byteLength"] = Value::from(64u64);
                }
            }
        }
    }
}

/// Re-assign Face and Hair mesh nodes to use the primary (Body) skin so that
/// SL sees a single, complete joint list for every mesh in the file.
///
/// When SL computes a joint's "position override" it works backwards through
/// the parent chain.  If a secondary mesh's own skin is missing ancestor joints
/// (e.g. Face skin has only mHead, no mNeck), SL falls back to its hard-coded
/// reference skeleton for the missing ancestors, producing a wrong local offset
/// for mHead and triggering "joint conflict" warnings.
///
/// The fix: patch Face/Hair mesh nodes so they reference the primary skin
/// (the body skin that already has the full 47-joint chain).  All vertices
/// in Face/Hair have been compacted to weight=1.0 slot-0 by
/// `soften_face_eye_influences`, so we only need to remap slot-0 → the slot
/// of mHead in the primary skin's joints array.
pub(super) fn merge_head_only_skins_into_primary(json: &mut Value, bin: &mut [u8]) {
    // ── 1. Find primary (Body) skin ──────────────────────────────────────────
    let skin_count = json
        .get("skins")
        .and_then(Value::as_array)
        .map(|s| s.len())
        .unwrap_or(0);
    if skin_count <= 1 {
        return;
    }

    let primary_skin_index = (0..skin_count)
        .max_by_key(|&i| {
            json["skins"][i]
                .get("joints")
                .and_then(Value::as_array)
                .map(|j| j.len())
                .unwrap_or(0)
        })
        .unwrap_or(0);

    // ── 2. Locate mHead in the primary skin's joints array ───────────────────
    let primary_joints: Vec<usize> = json["skins"][primary_skin_index]["joints"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default();

    let Some(mhead_slot_in_primary) = primary_joints.iter().position(|&node_idx| {
        json["nodes"][node_idx]
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            == "mHead"
    }) else {
        return; // Primary skin has no mHead – unexpected, bail out.
    };
    let new_slot = mhead_slot_in_primary as u16;

    // ── 3. For each non-primary skin that is head-only, migrate its meshes ───
    for skin_index in 0..skin_count {
        if skin_index == primary_skin_index {
            continue;
        }

        let joints: Vec<usize> = json["skins"][skin_index]["joints"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();

        // Only migrate skins that are already compacted to [mHead only].
        let is_mhead_only = joints.len() == 1
            && json["nodes"][joints[0]]
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                == "mHead";
        if !is_mhead_only {
            continue;
        }

        // Collect bindings BEFORE modifying the node skin references.
        let bindings = collect_skin_primitive_bindings(json, skin_index);

        // Remap vertex JOINTS_0 slot 0 → mhead_slot_in_primary.
        if new_slot != 0 {
            for binding in &bindings {
                let Some(joints_meta) = accessor_meta(json, binding.joints_accessor) else {
                    continue;
                };
                if !(joints_meta.component_type == 5121 || joints_meta.component_type == 5123) {
                    continue;
                }
                if joints_meta.accessor_type != "VEC4" {
                    continue;
                }
                for vertex_index in 0..joints_meta.count {
                    for lane in 0..4 {
                        // After soften_face_eye_influences all slots are 0.
                        // Write the new slot index that references mHead in
                        // the primary skin's joints array.
                        let _ = write_joint_slot(bin, &joints_meta, vertex_index, lane, new_slot);
                    }
                }
            }
        }

        // Change every mesh node using this skin to instead reference the primary skin.
        let node_count = json
            .get("nodes")
            .and_then(Value::as_array)
            .map(|n| n.len())
            .unwrap_or(0);
        for node_idx in 0..node_count {
            let node_skin = json["nodes"][node_idx]
                .get("skin")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            if node_skin == Some(skin_index) {
                if let Some(nodes_arr) = json.get_mut("nodes").and_then(Value::as_array_mut) {
                    nodes_arr[node_idx]["skin"] = Value::from(primary_skin_index as u64);
                }
            }
        }
    }
}

fn collect_skin_primitive_bindings(json: &Value, skin_index: usize) -> Vec<PrimitiveSkinBinding> {
    let nodes = json
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let meshes = json
        .get("meshes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut seen = HashSet::<(usize, usize)>::new();
    let mut bindings = Vec::<PrimitiveSkinBinding>::new();

    for node in nodes {
        let Some(node_skin_index) = node
            .get("skin")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
        else {
            continue;
        };
        if node_skin_index != skin_index {
            continue;
        }

        let Some(mesh_index) = node
            .get("mesh")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
        else {
            continue;
        };
        let Some(mesh) = meshes.get(mesh_index) else {
            continue;
        };
        let Some(primitives) = mesh.get("primitives").and_then(Value::as_array) else {
            continue;
        };

        for primitive in primitives {
            let Some(attributes) = primitive.get("attributes").and_then(Value::as_object) else {
                continue;
            };
            let Some(joints_accessor) = attributes
                .get("JOINTS_0")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
            else {
                continue;
            };
            let Some(weights_accessor) = attributes
                .get("WEIGHTS_0")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
            else {
                continue;
            };

            if seen.insert((joints_accessor, weights_accessor)) {
                bindings.push(PrimitiveSkinBinding {
                    joints_accessor,
                    weights_accessor,
                });
            }
        }
    }

    bindings
}

fn scan_used_joint_slots(
    json: &Value,
    bin: &[u8],
    binding: PrimitiveSkinBinding,
    used_slots: &mut [bool],
) {
    let Some(joints_meta) = accessor_meta(json, binding.joints_accessor) else {
        return;
    };
    let Some(weights_meta) = accessor_meta(json, binding.weights_accessor) else {
        return;
    };
    if joints_meta.accessor_type != "VEC4" || weights_meta.accessor_type != "VEC4" {
        return;
    }
    if !(joints_meta.component_type == 5121 || joints_meta.component_type == 5123) {
        return;
    }
    if weights_meta.component_type != 5126 {
        return;
    }

    let count = joints_meta.count.min(weights_meta.count);
    for vertex_index in 0..count {
        for lane in 0..4 {
            let weight = read_weight_f32(bin, &weights_meta, vertex_index, lane).unwrap_or(0.0);
            if weight <= 1e-6 {
                continue;
            }
            let slot = read_joint_slot(bin, &joints_meta, vertex_index, lane).unwrap_or(0) as usize;
            if slot < used_slots.len() {
                used_slots[slot] = true;
            }
        }
    }
}

fn remap_primitive_joints_and_weights(
    json: &Value,
    bin: &mut [u8],
    binding: PrimitiveSkinBinding,
    old_to_new: &[Option<u16>],
) {
    let Some(joints_meta) = accessor_meta(json, binding.joints_accessor) else {
        return;
    };
    let Some(weights_meta) = accessor_meta(json, binding.weights_accessor) else {
        return;
    };
    if joints_meta.accessor_type != "VEC4" || weights_meta.accessor_type != "VEC4" {
        return;
    }
    if !(joints_meta.component_type == 5121 || joints_meta.component_type == 5123) {
        return;
    }
    if weights_meta.component_type != 5126 {
        return;
    }

    let fallback = old_to_new.iter().flatten().copied().next().unwrap_or(0u16);

    let count = joints_meta.count.min(weights_meta.count);
    for vertex_index in 0..count {
        let mut slots = [0u16; 4];
        let mut weights = [0.0f32; 4];

        for lane in 0..4 {
            slots[lane] = read_joint_slot(bin, &joints_meta, vertex_index, lane).unwrap_or(0);
            weights[lane] = read_weight_f32(bin, &weights_meta, vertex_index, lane).unwrap_or(0.0);
        }

        for lane in 0..4 {
            let old_slot = slots[lane] as usize;
            if let Some(Some(mapped)) = old_to_new.get(old_slot) {
                slots[lane] = *mapped;
            } else {
                slots[lane] = fallback;
                weights[lane] = 0.0;
            }
        }

        let sum = weights.iter().sum::<f32>();
        if sum > 1e-8 {
            for weight in &mut weights {
                *weight /= sum;
            }
        } else {
            slots = [fallback, fallback, fallback, fallback];
            weights = [1.0, 0.0, 0.0, 0.0];
        }

        for lane in 0..4 {
            write_joint_slot(bin, &joints_meta, vertex_index, lane, slots[lane]);
            write_weight_f32(bin, &weights_meta, vertex_index, lane, weights[lane]);
        }
    }
}

fn compact_skin_joints_and_inverse_bind_matrices(
    json: &mut Value,
    bin: &mut [u8],
    skin_index: usize,
    keep_slots: &[usize],
) -> Result<()> {
    let joints_before = json
        .get("skins")
        .and_then(Value::as_array)
        .and_then(|skins| skins.get(skin_index))
        .and_then(|skin| skin.get("joints"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if joints_before.is_empty() {
        return Ok(());
    }

    let compacted_joints: Vec<Value> = keep_slots
        .iter()
        .filter_map(|slot| joints_before.get(*slot).cloned())
        .collect();

    let inverse_bind_accessor_index = json
        .get("skins")
        .and_then(Value::as_array)
        .and_then(|skins| skins.get(skin_index))
        .and_then(|skin| skin.get("inverseBindMatrices"))
        .and_then(Value::as_u64)
        .map(|value| value as usize);

    if let Some(accessor_index) = inverse_bind_accessor_index {
        compact_inverse_bind_accessor(json, bin, accessor_index, keep_slots)?;
    }

    if let Some(skins) = json.get_mut("skins").and_then(Value::as_array_mut)
        && let Some(skin) = skins.get_mut(skin_index)
    {
        skin["joints"] = Value::Array(compacted_joints);
    }

    Ok(())
}

fn compact_inverse_bind_accessor(
    json: &mut Value,
    bin: &mut [u8],
    accessor_index: usize,
    keep_slots: &[usize],
) -> Result<()> {
    let Some(meta) = accessor_meta(json, accessor_index) else {
        return Ok(());
    };
    if meta.accessor_type != "MAT4" || meta.component_type != 5126 {
        return Ok(());
    }

    let old_count = meta.count;
    let stride = meta.stride.max(64);
    let mut matrices = Vec::<[u8; 64]>::new();

    for slot in keep_slots.iter().copied() {
        if slot >= old_count {
            continue;
        }
        let offset = meta.base_offset + slot * stride;
        if offset + 64 > bin.len() {
            continue;
        }
        let mut bytes = [0u8; 64];
        bytes.copy_from_slice(&bin[offset..offset + 64]);
        matrices.push(bytes);
    }

    for (index, bytes) in matrices.iter().enumerate() {
        let offset = meta.base_offset + index * stride;
        if offset + 64 > bin.len() {
            break;
        }
        bin[offset..offset + 64].copy_from_slice(bytes);
    }

    // Read the bufferView index before taking mutable borrows.
    let bv_index = json
        .get("accessors")
        .and_then(Value::as_array)
        .and_then(|accs| accs.get(accessor_index))
        .and_then(|acc| acc.get("bufferView"))
        .and_then(Value::as_u64)
        .map(|v| v as usize);

    let Some(accessors) = json.get_mut("accessors").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    let Some(accessor) = accessors.get_mut(accessor_index) else {
        return Ok(());
    };
    accessor["count"] = Value::from(matrices.len() as u64);
    let _ = accessors; // end the mutable borrow before borrowing bufferViews

    // Trim the bufferView byteLength to exactly the bytes now occupied so
    // that importers (e.g. the SL uploader) that read byteLength/64 matrices
    // instead of the accessor count don't consume stale IBM data beyond the
    // end of the compacted block.
    if let Some(bv_idx) = bv_index
        && let Some(bv_arr) = json.get_mut("bufferViews").and_then(Value::as_array_mut)
        && let Some(bv) = bv_arr.get_mut(bv_idx)
    {
        let needed = matrices.len() * stride;
        let current = bv.get("byteLength").and_then(Value::as_u64).unwrap_or(0) as usize;
        if needed < current {
            bv["byteLength"] = Value::from(needed as u64);
        }
    }

    Ok(())
}
