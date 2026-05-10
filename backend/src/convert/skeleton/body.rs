use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use nalgebra::{Matrix4, Vector3};
use serde_json::Value;

use super::super::gltf_utils::{
    collect_node_name_set_from_json, collect_parent_index_map_from_json,
    compute_node_world_matrices, node_to_local_matrix, set_node_local_matrix, write_mat4_f32_le,
};
use super::super::types::{
    BENTO_BONE_MAP, BENTO_HIERARCHY_RELATIONS, BONE_MAP, CORE_HIERARCHY_RELATIONS, ValidationIssue,
};

pub(in super::super) fn rename_bones(
    json: &mut Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
) {
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

pub(in super::super) fn reconstruct_sl_core_hierarchy(
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

    let planned_links: Vec<(usize, usize)> = {
        let mut child_to_parent: HashMap<usize, usize> = HashMap::new();
        for (parent, child) in CORE_HIERARCHY_RELATIONS
            .iter()
            .chain(BENTO_HIERARCHY_RELATIONS.iter())
        {
            let Some(parent_index) = humanoid_bone_nodes.get(*parent).copied() else {
                continue;
            };
            let Some(child_index) = humanoid_bone_nodes.get(*child).copied() else {
                continue;
            };
            if parent_index == child_index {
                continue;
            }
            child_to_parent.insert(child_index, parent_index);
        }
        child_to_parent
            .into_iter()
            .map(|(child, parent)| (parent, child))
            .collect()
    };

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

pub(in super::super) fn validate_bone_conversion_preconditions(
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
                    severity: super::super::types::Severity::Error,
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

pub(in super::super) fn ensure_target_bones_exist_after_rename(
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

pub(in super::super) fn promote_pelvis_to_scene_root(
    json: &mut Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
) -> Option<usize> {
    let Some(pelvis_index) = humanoid_bone_nodes.get("hips").copied() else {
        return None;
    };

    let parent_map = collect_parent_index_map_from_json(json);

    let mut ancestors: Vec<usize> = Vec::new();
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
        ancestors.push(parent_idx);
        current = parent_idx;
    }

    if ancestors.is_empty() {
        return None;
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
    let pelvis_world_t = Vector3::new(
        pelvis_world[(0, 3)],
        pelvis_world[(1, 3)],
        pelvis_world[(2, 3)],
    );

    let identity_root_index = *ancestors.last().unwrap();
    let intermediates: HashSet<usize> = ancestors
        .iter()
        .copied()
        .filter(|&idx| idx != identity_root_index)
        .collect();

    if let Some(obj) = json["nodes"][identity_root_index].as_object_mut() {
        obj.remove("matrix");
        obj.insert(
            "translation".to_string(),
            serde_json::json!([0.0, 0.0, 0.0]),
        );
        obj.insert(
            "rotation".to_string(),
            serde_json::json!([0.0, 0.0, 0.0, 1.0]),
        );
        obj.insert("scale".to_string(), serde_json::json!([1.0, 1.0, 1.0]));
    }

    if let Some(direct_parent) = parent_map.get(&pelvis_index).copied() {
        if let Some(dp_node) = json["nodes"][direct_parent].as_object_mut() {
            if let Some(Value::Array(children)) = dp_node.get_mut("children") {
                children.retain(|v| v.as_u64().map(|n| n as usize) != Some(pelvis_index));
            }
        }
    }

    let mut nodes_to_promote: Vec<usize> = Vec::new();
    for &ancestor_idx in &ancestors {
        let children_snapshot: Vec<usize> = json["nodes"][ancestor_idx]
            .get("children")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();
        for child_idx in children_snapshot {
            if child_idx != pelvis_index
                && child_idx != identity_root_index
                && !intermediates.contains(&child_idx)
            {
                nodes_to_promote.push(child_idx);
            }
        }
    }

    for &inter_idx in &intermediates {
        if let Some(obj) = json["nodes"][inter_idx].as_object_mut() {
            obj.remove("children");
        }
    }

    if let Some(obj) = json["nodes"][identity_root_index].as_object_mut() {
        obj.insert(
            "children".to_string(),
            serde_json::json!([pelvis_index as u64]),
        );
    }

    if let Some(node_obj) = json["nodes"][pelvis_index].as_object_mut() {
        node_obj.remove("matrix");
        node_obj.insert(
            "translation".to_string(),
            serde_json::json!([pelvis_world_t.x, pelvis_world_t.y, pelvis_world_t.z]),
        );
        if !node_obj.contains_key("rotation") {
            node_obj.insert(
                "rotation".to_string(),
                serde_json::json!([0.0, 0.0, 0.0, 1.0]),
            );
        }
    }

    if let Some(scene) = json
        .get_mut("scenes")
        .and_then(Value::as_array_mut)
        .and_then(|s| s.first_mut())
    {
        if let Some(Value::Array(scene_nodes)) = scene.get_mut("nodes") {
            scene_nodes.retain(|v| {
                v.as_u64()
                    .map(|n| {
                        let idx = n as usize;
                        !intermediates.contains(&idx) && idx != pelvis_index
                    })
                    .unwrap_or(true)
            });

            let root_val = Value::from(identity_root_index as u64);
            if !scene_nodes.iter().any(|v| v == &root_val) {
                scene_nodes.push(root_val);
            }

            for promote_idx in nodes_to_promote {
                let val = Value::from(promote_idx as u64);
                if !scene_nodes.iter().any(|v| v == &val) {
                    scene_nodes.push(val);
                }
            }
        }
    }

    Some(identity_root_index)
}

pub(in super::super) fn set_skin_skeleton_root(
    json: &mut Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
    identity_root: Option<usize>,
) {
    let skeleton_index = humanoid_bone_nodes.get("hips").copied().or(identity_root);

    let skins = match json["skins"].as_array_mut() {
        Some(s) => s,
        None => return,
    };

    for skin in skins.iter_mut() {
        if let Some(idx) = skeleton_index {
            skin["skeleton"] = Value::Number(idx.into());
            continue;
        }

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

pub(in super::super) fn regenerate_inverse_bind_matrices(
    json: &mut Value,
    bin: &mut [u8],
) -> Result<()> {
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
