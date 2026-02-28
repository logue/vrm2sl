use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use nalgebra::{Matrix4, Vector3};
use serde_json::Value;

use super::gltf_utils::{
    collect_node_name_set_from_json, collect_parent_index_map_from_json,
    compute_node_world_matrices, node_to_local_matrix, set_node_local_matrix, write_mat4_f32_le,
};
use super::types::{
    BENTO_BONE_MAP, BENTO_HIERARCHY_RELATIONS, BONE_MAP, CORE_HIERARCHY_RELATIONS, ValidationIssue,
};

// ─── Bone renaming ────────────────────────────────────────────────────────────

/// Rename known bones according to the VRM→SL mapping table.
pub(super) fn rename_bones(json: &mut Value, humanoid_bone_nodes: &HashMap<String, usize>) {
    if let Some(nodes) = json.get_mut("nodes").and_then(Value::as_array_mut) {
        for (source, target) in BONE_MAP.iter().chain(BENTO_BONE_MAP.iter()) {
            if let Some(node_index) = humanoid_bone_nodes.get(*source).copied() {
                if let Some(node) = nodes.get_mut(node_index) {
                    node["name"] = Value::String(target.to_string());
                }
            }
        }
    }
}

// ─── Hierarchy reconstruction ─────────────────────────────────────────────────

/// Reconstruct core humanoid hierarchy toward SL-compatible parent-child links.
pub(super) fn reconstruct_sl_core_hierarchy(
    json: &mut Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
) {
    let original_node_locals: Vec<Matrix4<f32>> = json
        .get("nodes")
        .and_then(Value::as_array)
        .map(|nodes| nodes.iter().map(node_to_local_matrix).collect())
        .unwrap_or_default();
    let original_parent_map = collect_parent_index_map_from_json(json);
    let original_node_worlds =
        compute_node_world_matrices(&original_node_locals, &original_parent_map);

    let planned_links: Vec<(usize, usize)> = CORE_HIERARCHY_RELATIONS
        .iter()
        .chain(BENTO_HIERARCHY_RELATIONS.iter())
        .filter_map(|(parent, child)| {
            let parent_index = humanoid_bone_nodes.get(*parent).copied()?;
            let child_index = humanoid_bone_nodes.get(*child).copied()?;
            if parent_index == child_index {
                return None;
            }
            Some((parent_index, child_index))
        })
        .collect();

    if planned_links.is_empty() {
        return;
    }

    let controlled_children: HashSet<usize> =
        planned_links.iter().map(|(_, child)| *child).collect();
    let planned_parent_map: HashMap<usize, usize> = planned_links
        .iter()
        .map(|(parent, child)| (*child, *parent))
        .collect();

    if let Some(nodes) = json.get_mut("nodes").and_then(Value::as_array_mut) {
        for node in nodes.iter_mut() {
            if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
                children.retain(|child| {
                    child
                        .as_u64()
                        .map(|index| !controlled_children.contains(&(index as usize)))
                        .unwrap_or(true)
                });
            }
        }

        for (parent_index, child_index) in planned_links {
            if let Some(parent_node) = nodes.get_mut(parent_index) {
                let children = parent_node.as_object_mut().map(|object| {
                    object
                        .entry("children")
                        .or_insert_with(|| Value::Array(vec![]))
                });

                if let Some(Value::Array(children)) = children {
                    let child_value = Value::from(child_index as u64);
                    if !children.iter().any(|entry| entry == &child_value) {
                        children.push(child_value);
                    }
                }
            }
        }

        for (child_index, parent_index) in planned_parent_map {
            let (Some(parent_world), Some(child_world)) = (
                original_node_worlds.get(parent_index),
                original_node_worlds.get(child_index),
            ) else {
                continue;
            };

            let Some(new_local) = parent_world.try_inverse().map(|inv| inv * child_world) else {
                continue;
            };

            if let Some(child_node) = nodes.get_mut(child_index) {
                set_node_local_matrix(child_node, &new_local);
            }
        }
    }

    if let Some(first_scene_nodes) = json
        .get_mut("scenes")
        .and_then(Value::as_array_mut)
        .and_then(|scenes| scenes.first_mut())
        .and_then(|scene| scene.get_mut("nodes"))
        .and_then(Value::as_array_mut)
    {
        first_scene_nodes.retain(|node| {
            node.as_u64()
                .map(|index| !controlled_children.contains(&(index as usize)))
                .unwrap_or(true)
        });
    }
}

// ─── Bone precondition validation ─────────────────────────────────────────────

/// Validate that required source bones point to valid node indices before conversion.
pub(super) fn validate_bone_conversion_preconditions(
    json: &Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
) -> Vec<ValidationIssue> {
    let node_count = json
        .get("nodes")
        .and_then(Value::as_array)
        .map(|nodes| nodes.len())
        .unwrap_or(0);

    BONE_MAP
        .iter()
        .chain(BENTO_BONE_MAP.iter())
        .filter_map(|(source, _)| {
            let Some(node_index) = humanoid_bone_nodes.get(*source).copied() else {
                return None;
            };

            if node_index >= node_count {
                return Some(ValidationIssue {
                    severity: super::types::Severity::Error,
                    code: "INVALID_BONE_NODE_INDEX".to_string(),
                    message: format!(
                        "[ERROR] Bone conversion precondition failed: '{}' points to invalid node index {} (node count: {})",
                        source, node_index, node_count
                    ),
                });
            }

            None
        })
        .collect()
}

/// Ensure all expected target SL bone names exist after rename.
pub(super) fn ensure_target_bones_exist_after_rename(
    json: &Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
) -> Result<()> {
    let node_name_set = collect_node_name_set_from_json(json);

    let expected_targets: Vec<String> = BONE_MAP
        .iter()
        .chain(BENTO_BONE_MAP.iter())
        .filter(|(source, _)| humanoid_bone_nodes.contains_key(*source))
        .map(|(_, target)| target.to_string())
        .collect();

    let missing_targets: Vec<String> = expected_targets
        .into_iter()
        .filter(|target| !node_name_set.contains(target))
        .collect();

    if !missing_targets.is_empty() {
        bail!(
            "[ERROR] Bone conversion incomplete: missing target SL bone names after rename: {}",
            missing_targets.join(", ")
        );
    }

    Ok(())
}

// ─── Rotation normalisation ───────────────────────────────────────────────────

/// Normalize the local rotation of every SL-mapped bone to identity while
/// preserving the bone's world-space position.
///
/// Second Life reads joint bind-positions from the inverse-bind-matrix
/// (4th column) and then applies its **own** default (identity) orientations
/// when deforming the mesh. Any non-identity local rotation baked into the
/// glTF node hierarchy will therefore cause incorrect deformation because the
/// IBM accounts for the rotation but SL does not re-apply it.
///
/// Bones are processed in topological (parent-before-child) order so that each
/// child uses the already-corrected parent world position when computing its
/// own local translation.
pub(super) fn normalize_sl_bone_rotations(
    json: &mut Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
) {
    let sl_node_indices: HashSet<usize> = BONE_MAP
        .iter()
        .chain(BENTO_BONE_MAP.iter())
        .filter_map(|(vrm_name, _)| humanoid_bone_nodes.get(*vrm_name).copied())
        .collect();

    let node_locals: Vec<Matrix4<f32>> = json["nodes"]
        .as_array()
        .map(|nodes| nodes.iter().map(node_to_local_matrix).collect())
        .unwrap_or_default();
    let parent_map = collect_parent_index_map_from_json(json);
    let node_worlds_snapshot = compute_node_world_matrices(&node_locals, &parent_map);

    let node_count = json["nodes"].as_array().map(|a| a.len()).unwrap_or(0);

    // BFS topological order (parents before children).
    let mut topo_order: Vec<usize> = Vec::with_capacity(node_count);
    {
        let mut queue: std::collections::VecDeque<usize> = (0..node_count)
            .filter(|&i| !parent_map.contains_key(&i))
            .collect();
        while let Some(idx) = queue.pop_front() {
            topo_order.push(idx);
            if let Some(children) = json["nodes"][idx].get("children").and_then(Value::as_array) {
                for child in children {
                    if let Some(c) = child.as_u64().map(|v| v as usize) {
                        queue.push_back(c);
                    }
                }
            }
        }
    }

    let mut effective_world_t: Vec<Vector3<f32>> = node_worlds_snapshot
        .iter()
        .map(|m| Vector3::new(m[(0, 3)], m[(1, 3)], m[(2, 3)]))
        .collect();
    while effective_world_t.len() < node_count {
        effective_world_t.push(Vector3::zeros());
    }

    for &node_idx in &topo_order {
        if !sl_node_indices.contains(&node_idx) {
            continue;
        }

        let snapshot_t = match node_worlds_snapshot.get(node_idx) {
            Some(m) => Vector3::new(m[(0, 3)], m[(1, 3)], m[(2, 3)]),
            None => continue,
        };

        let parent_effective_t = match parent_map.get(&node_idx) {
            Some(&parent_idx) => effective_world_t[parent_idx],
            None => Vector3::zeros(),
        };

        let local_t = snapshot_t - parent_effective_t;
        effective_world_t[node_idx] = parent_effective_t + local_t;

        let scale = json["nodes"][node_idx]
            .get("scale")
            .and_then(Value::as_array)
            .and_then(|a| {
                let sx = a.first().and_then(Value::as_f64)? as f32;
                let sy = a.get(1).and_then(Value::as_f64)? as f32;
                let sz = a.get(2).and_then(Value::as_f64)? as f32;
                Some([sx, sy, sz])
            })
            .unwrap_or([1.0, 1.0, 1.0]);

        if let Some(obj) = json["nodes"][node_idx].as_object_mut() {
            obj.remove("matrix");
            obj.insert(
                "translation".to_string(),
                serde_json::json!([local_t.x, local_t.y, local_t.z]),
            );
            obj.insert(
                "rotation".to_string(),
                serde_json::json!([0.0, 0.0, 0.0, 1.0]),
            );
            obj.insert(
                "scale".to_string(),
                serde_json::json!([scale[0], scale[1], scale[2]]),
            );
        }
    }
}

// ─── Scene root promotion ─────────────────────────────────────────────────────

/// Promote `mPelvis` to become a direct child of the scene, removing any
/// intermediate wrapper nodes (e.g. the "Root" empty that VRM files typically
/// insert above the skeleton).
///
/// VRM fields commonly have the structure:
///   scene → Root (node 0, sometimes with a non-identity transform)
///               └─ Armature / J_Root
///                      └─ mPelvis → …
///
/// When a glTF skin's inverse-bind-matrices are computed relative to the world
/// origin, any non-identity ancestor transform above mPelvis will shift every
/// joint's world matrix.  In the Second Life viewer this manifests as deformed
/// or offset limbs.
pub(super) fn promote_pelvis_to_scene_root(
    json: &mut Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
) {
    let Some(pelvis_index) = humanoid_bone_nodes.get("hips").copied() else {
        return;
    };

    let parent_map = collect_parent_index_map_from_json(json);

    let mut ancestors_to_remove: Vec<usize> = Vec::new();
    let mut current = pelvis_index;
    while let Some(&parent_idx) = parent_map.get(&current) {
        let parent_name = json["nodes"][parent_idx]
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let is_sl_bone = parent_name.starts_with('m')
            || humanoid_bone_nodes.values().any(|&idx| idx == parent_idx);
        if is_sl_bone {
            break;
        }
        ancestors_to_remove.push(parent_idx);
        current = parent_idx;
    }

    if ancestors_to_remove.is_empty() {
        return;
    }

    let node_locals: Vec<Matrix4<f32>> = json["nodes"]
        .as_array()
        .map(|nodes| nodes.iter().map(node_to_local_matrix).collect())
        .unwrap_or_default();
    let world_matrices = compute_node_world_matrices(&node_locals, &parent_map);

    let pelvis_world = world_matrices
        .get(pelvis_index)
        .copied()
        .unwrap_or_else(Matrix4::<f32>::identity);

    let new_local_t = Vector3::new(
        pelvis_world[(0, 3)],
        pelvis_world[(1, 3)],
        pelvis_world[(2, 3)],
    );

    // 1. Remove mPelvis from its direct parent's children list.
    let direct_parent = parent_map.get(&pelvis_index).copied();
    if let Some(dp) = direct_parent {
        if let Some(dp_node) = json["nodes"][dp].as_object_mut() {
            if let Some(Value::Array(children)) = dp_node.get_mut("children") {
                children.retain(|v| v.as_u64().map(|n| n as usize) != Some(pelvis_index));
            }
        }
    }

    // 2. Update mPelvis local translation to its world position (no parent anymore).
    if let Some(node_obj) = json["nodes"][pelvis_index].as_object_mut() {
        node_obj.remove("matrix");
        node_obj.insert(
            "translation".to_string(),
            serde_json::json!([new_local_t.x, new_local_t.y, new_local_t.z]),
        );
        if !node_obj.contains_key("rotation") {
            node_obj.insert(
                "rotation".to_string(),
                serde_json::json!([0.0, 0.0, 0.0, 1.0]),
            );
        }
    }

    // 3. Remove wrapper ancestors from the scene node list; add mPelvis.
    let ancestors_set: HashSet<usize> = ancestors_to_remove.iter().copied().collect();
    if let Some(scene) = json
        .get_mut("scenes")
        .and_then(Value::as_array_mut)
        .and_then(|s| s.first_mut())
    {
        if let Some(Value::Array(scene_nodes)) = scene.get_mut("nodes") {
            scene_nodes.retain(|v| {
                v.as_u64()
                    .map(|n| !ancestors_set.contains(&(n as usize)))
                    .unwrap_or(true)
            });
            let pelvis_val = Value::from(pelvis_index as u64);
            if !scene_nodes.iter().any(|v| v == &pelvis_val) {
                scene_nodes.push(pelvis_val);
            }
        }
    }

    // 4. Promote non-skeleton children of removed wrappers to the scene list
    //    so that mesh nodes (Body/Face/Hair) are not orphaned.
    let mut nodes_to_promote: Vec<usize> = Vec::new();
    for &ancestor_idx in &ancestors_to_remove {
        if let Some(Value::Array(children)) = json["nodes"][ancestor_idx].get("children") {
            for child_val in children.clone() {
                if let Some(child_idx) = child_val.as_u64().map(|n| n as usize) {
                    if child_idx != pelvis_index && !ancestors_set.contains(&child_idx) {
                        nodes_to_promote.push(child_idx);
                    }
                }
            }
        }
    }
    if let Some(scene) = json
        .get_mut("scenes")
        .and_then(Value::as_array_mut)
        .and_then(|s| s.first_mut())
    {
        if let Some(Value::Array(scene_nodes)) = scene.get_mut("nodes") {
            for promote_idx in nodes_to_promote {
                let val = Value::from(promote_idx as u64);
                if !scene_nodes.iter().any(|v| v == &val) {
                    scene_nodes.push(val);
                }
            }
        }
    }
}

// ─── skin.skeleton ────────────────────────────────────────────────────────────

/// Set `skin.skeleton` for every skin to the `mPelvis` node (hips in SL).
///
/// The glTF spec defines `skin.skeleton` as "the index of the node used as a
/// skeleton root."  It does NOT require the skeleton root to be listed in the
/// skin's `joints` array.  Setting it to `mPelvis` for ALL skins — even those
/// whose joints are limited to head/eye bones — ensures that every viewer
/// (including the SL viewer) starts joint-world-transform traversal from the
/// correct scene-root node instead of from the first joint's local position.
///
/// Without this fix, a Face/Hair skin whose first joint is `mHead` would have
/// `skeleton: mHead`, and viewers that start traversal from `skeleton` would
/// compute `mHead`'s world matrix using only its local translation (≈ 0.09 m)
/// rather than the full parent chain through `mPelvis` (≈ 1.73 m), causing the
/// head mesh to appear near the avatar's feet.
pub(super) fn set_skin_skeleton_to_pelvis(
    json: &mut Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
) {
    let pelvis_index = humanoid_bone_nodes.get("hips").copied();

    let skins = match json["skins"].as_array_mut() {
        Some(s) => s,
        None => return,
    };

    for skin in skins.iter_mut() {
        // Always set skeleton to mPelvis when available, even when mPelvis is
        // not listed in this skin's joints array.  glTF allows the skeleton
        // root to be an ancestor of all joints rather than one of the joints.
        if let Some(pelvis_idx) = pelvis_index {
            skin["skeleton"] = Value::Number(pelvis_idx.into());
            continue;
        }

        // Fallback (no hips bone found): use the first joint as skeleton root.
        let joints: Vec<usize> = skin["joints"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();

        if let Some(&first) = joints.first() {
            skin["skeleton"] = Value::Number(first.into());
        }
    }
}

// ─── Inverse bind matrix regeneration ────────────────────────────────────────

/// Rebuild inverse bind matrices from current node transforms and write them
/// back to the binary buffer for all skins that have writable MAT4 float accessors.
pub(super) fn regenerate_inverse_bind_matrices(json: &mut Value, bin: &mut [u8]) -> Result<()> {
    if bin.is_empty() {
        return Ok(());
    }

    let Some(nodes_json) = json.get("nodes").and_then(Value::as_array) else {
        return Ok(());
    };

    let node_locals: Vec<Matrix4<f32>> = nodes_json.iter().map(node_to_local_matrix).collect();
    let parent_map = collect_parent_index_map_from_json(json);
    let node_worlds = compute_node_world_matrices(&node_locals, &parent_map);

    let Some(skins) = json.get("skins").and_then(Value::as_array) else {
        return Ok(());
    };

    let accessors = json
        .get("accessors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let buffer_views = json
        .get("bufferViews")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for skin in skins {
        let Some(joints) = skin.get("joints").and_then(Value::as_array) else {
            continue;
        };

        let Some(accessor_index) = skin
            .get("inverseBindMatrices")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
        else {
            continue;
        };

        let Some(accessor) = accessors.get(accessor_index) else {
            continue;
        };

        let component_type = accessor
            .get("componentType")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let accessor_type = accessor
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if component_type != 5126 || accessor_type != "MAT4" {
            continue;
        }

        let Some(buffer_view_index) = accessor
            .get("bufferView")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
        else {
            continue;
        };

        let Some(buffer_view) = buffer_views.get(buffer_view_index) else {
            continue;
        };

        let view_offset = buffer_view
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let accessor_offset = accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let base_offset = view_offset.saturating_add(accessor_offset);
        let stride = buffer_view
            .get("byteStride")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(64);

        for (joint_array_index, joint) in joints.iter().enumerate() {
            let Some(joint_index) = joint.as_u64().map(|value| value as usize) else {
                continue;
            };
            let Some(world) = node_worlds.get(joint_index) else {
                continue;
            };

            let inverse = world.try_inverse().unwrap_or_else(Matrix4::<f32>::identity);
            let write_offset = base_offset.saturating_add(joint_array_index.saturating_mul(stride));
            if write_offset + 64 > bin.len() {
                continue;
            }
            write_mat4_f32_le(bin, write_offset, &inverse);
        }
    }

    Ok(())
}
