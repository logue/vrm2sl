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

    let Some(accessors) = json.get_mut("accessors").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    let Some(accessor) = accessors.get_mut(accessor_index) else {
        return Ok(());
    };
    accessor["count"] = Value::from(matrices.len() as u64);

    Ok(())
}
