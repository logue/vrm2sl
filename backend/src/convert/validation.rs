use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use gltf::Document;
use serde_json::Value;

use super::types::{
    BENTO_BONE_MAP, BONE_MAP, REQUIRED_BONES, Severity, TextureInfo, UploadFeeEstimate,
    ValidationIssue,
};

// ─── Required parent-child relationships ──────────────────────────────────────

const REQUIRED_PARENT_RELATIONS: [(&str, &str); 12] = [
    ("hips", "spine"),
    ("spine", "chest"),
    ("chest", "neck"),
    ("neck", "head"),
    ("leftUpperArm", "leftLowerArm"),
    ("leftLowerArm", "leftHand"),
    ("rightUpperArm", "rightLowerArm"),
    ("rightLowerArm", "rightHand"),
    ("leftUpperLeg", "leftLowerLeg"),
    ("leftLowerLeg", "leftFoot"),
    ("rightUpperLeg", "rightLowerLeg"),
    ("rightLowerLeg", "rightFoot"),
];

// ─── Source model validation ──────────────────────────────────────────────────

/// Validate that the source appears to be a supported VRoid/VRM model.
pub(super) fn validate_vroid_model(json: &Value) -> Result<()> {
    let generator = json
        .get("asset")
        .and_then(|asset| asset.get("generator"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    let vrm_extension_present = json
        .get("extensions")
        .and_then(Value::as_object)
        .map(|ext| {
            ext.keys()
                .any(|key| key.to_ascii_lowercase().contains("vrm"))
        })
        .unwrap_or(false);

    if generator.to_ascii_lowercase().contains("vroid") {
        return Ok(());
    }

    if vrm_extension_present {
        return Ok(());
    }

    bail!("[ERROR] Only standard VRoid Studio VRM files are supported")
}

/// Collect all named node identifiers from the source document.
pub(super) fn collect_node_names(document: &Document) -> HashSet<String> {
    document
        .nodes()
        .filter_map(|node| node.name().map(ToOwned::to_owned))
        .collect()
}

/// Build a child→parent node-index map for hierarchy validation.
pub(super) fn collect_parent_index_map(document: &Document) -> HashMap<usize, usize> {
    let mut parent_map = HashMap::new();
    for parent in document.nodes() {
        for child in parent.children() {
            parent_map.insert(child.index(), parent.index());
        }
    }
    parent_map
}

/// Return missing required bones from the humanoid bone node map.
pub(super) fn collect_missing_required_bones(
    humanoid_bone_nodes: &HashMap<String, usize>,
) -> Vec<String> {
    REQUIRED_BONES
        .iter()
        .filter(|bone_name| !humanoid_bone_nodes.contains_key(**bone_name))
        .map(|bone_name| bone_name.to_string())
        .collect()
}

/// Validate required humanoid hierarchy relationships.
pub(super) fn validate_hierarchy(
    humanoid_bone_nodes: &HashMap<String, usize>,
    parent_map: &HashMap<usize, usize>,
) -> Vec<ValidationIssue> {
    REQUIRED_PARENT_RELATIONS
        .iter()
        .filter_map(|(parent, child)| {
            let Some(parent_index) = humanoid_bone_nodes.get(*parent).copied() else {
                return None;
            };
            let Some(child_index) = humanoid_bone_nodes.get(*child).copied() else {
                return None;
            };

            let Some(actual_parent_index) = parent_map.get(&child_index).copied() else {
                return Some(ValidationIssue {
                    severity: Severity::Error,
                    code: "INVALID_BONE_HIERARCHY".to_string(),
                    message: format!(
                        "[ERROR] Non-standard bone hierarchy: parent for '{}' is not set",
                        child
                    ),
                });
            };

            let is_valid_parent = if *parent == "chest" && *child == "neck" {
                let upper_chest_index = humanoid_bone_nodes.get("upperChest").copied();
                actual_parent_index == parent_index
                    || upper_chest_index
                        .map(|index| index == actual_parent_index)
                        .unwrap_or(false)
            } else {
                actual_parent_index == parent_index
            };

            if !is_valid_parent {
                return Some(ValidationIssue {
                    severity: Severity::Error,
                    code: "INVALID_BONE_HIERARCHY".to_string(),
                    message: format!(
                        "[ERROR] Non-standard bone hierarchy: '{}' parent index is {} (expected {})",
                        child, actual_parent_index, parent_index
                    ),
                });
            }

            None
        })
        .collect()
}

// ─── Bone mapping helpers ─────────────────────────────────────────────────────

/// Return mapped source→target bone pairs present in the input model.
pub(super) fn collect_mapped_bones(
    humanoid_bone_nodes: &HashMap<String, usize>,
) -> Vec<(String, String)> {
    BONE_MAP
        .iter()
        .chain(BENTO_BONE_MAP.iter())
        .filter(|(source, _)| humanoid_bone_nodes.contains_key(*source))
        .map(|(source, target)| (source.to_string(), target.to_string()))
        .collect()
}

/// Extract humanoid-bone semantic to node-index mapping from VRM extensions.
pub(super) fn extract_humanoid_bone_nodes(json: &Value) -> HashMap<String, usize> {
    let mut mapping = HashMap::<String, usize>::new();

    if let Some(vrmc_humanoid) = json
        .pointer("/extensions/VRMC_vrm/humanoid/humanBones")
        .and_then(Value::as_object)
    {
        for (bone_name, value) in vrmc_humanoid {
            if let Some(node_index) = value
                .get("node")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
            {
                mapping.insert(bone_name.clone(), node_index);
            }
        }
    }

    if let Some(vrm_humanoid) = json
        .pointer("/extensions/VRM/humanoid/humanBones")
        .and_then(Value::as_array)
    {
        for value in vrm_humanoid {
            let bone_name = value
                .get("bone")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            let node_index = value
                .get("node")
                .and_then(Value::as_u64)
                .map(|node| node as usize);

            if let (Some(bone_name), Some(node_index)) = (bone_name, node_index) {
                mapping.entry(bone_name).or_insert(node_index);
            }
        }
    }

    mapping
}

// ─── Metadata extraction ──────────────────────────────────────────────────────

/// Extract model name from VRM metadata or asset generator.
pub(super) fn extract_model_name(json: &Value) -> Option<String> {
    json.pointer("/extensions/VRM/meta/name")
        .or_else(|| json.pointer("/extensions/VRMC_vrm/meta/name"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            json.get("asset")
                .and_then(|asset| asset.get("generator"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
}

/// Extract author/copyright metadata from VRM or asset section.
pub(super) fn extract_author(json: &Value) -> Option<String> {
    json.pointer("/extensions/VRM/meta/authors/0")
        .or_else(|| json.pointer("/extensions/VRMC_vrm/meta/authors/0"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            json.get("asset")
                .and_then(|asset| asset.get("copyright"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
}

// ─── Texture fee estimation ───────────────────────────────────────────────────

/// Estimate texture upload fees before/after resize policy.
pub(super) fn estimate_texture_fee(
    texture_infos: &[TextureInfo],
    auto_resize_to_1024: bool,
) -> UploadFeeEstimate {
    let before = texture_infos
        .iter()
        .map(|texture| fee_per_texture(texture.width, texture.height))
        .sum::<u32>();

    let after = texture_infos
        .iter()
        .map(|texture| {
            let (projected_width, projected_height) =
                projected_texture_size(texture.width, texture.height, auto_resize_to_1024);
            fee_per_texture(projected_width, projected_height)
        })
        .sum::<u32>();

    let reduction_percent = if before > 0 {
        ((before.saturating_sub(after)) * 100) / before
    } else {
        0
    };

    UploadFeeEstimate {
        before_linden_dollar: before,
        after_resize_linden_dollar: after,
        reduction_percent,
    }
}

/// Estimate texture size after export policy is applied.
pub(super) fn projected_texture_size(
    width: u32,
    height: u32,
    auto_resize_to_1024: bool,
) -> (u32, u32) {
    let max_dim = width.max(height);
    let target_max = if max_dim <= 1024 {
        1024
    } else if max_dim <= 2048 {
        if auto_resize_to_1024 { 1024 } else { max_dim }
    } else if auto_resize_to_1024 {
        1024
    } else {
        2048
    };

    if max_dim <= target_max {
        return (width, height);
    }

    let scale = target_max as f64 / max_dim as f64;
    let projected_width = (width as f64 * scale).round().max(1.0) as u32;
    let projected_height = (height as f64 * scale).round().max(1.0) as u32;
    (projected_width, projected_height)
}

/// Estimate fee per texture based on max dimension bands.
pub(super) fn fee_per_texture(width: u32, height: u32) -> u32 {
    let max_dim = width.max(height);
    if max_dim <= 512 {
        10
    } else if max_dim <= 1024 {
        20
    } else if max_dim <= 2048 {
        50
    } else {
        100
    }
}

// ─── Extension / extras cleanup ───────────────────────────────────────────────

/// Remove VRM-specific extensions and recursive extras fields.
pub(super) fn remove_vrm_extensions_and_extras(json: &mut Value) {
    if let Some(top_extensions) = json.get_mut("extensions").and_then(Value::as_object_mut) {
        let keys: Vec<String> = top_extensions
            .keys()
            .filter(|key| key.to_ascii_lowercase().contains("vrm"))
            .cloned()
            .collect();
        for key in keys {
            top_extensions.remove(&key);
        }
    }

    if let Some(extensions_used) = json.get_mut("extensionsUsed").and_then(Value::as_array_mut) {
        extensions_used.retain(|entry| {
            entry
                .as_str()
                .map(|name| !name.to_ascii_lowercase().contains("vrm"))
                .unwrap_or(true)
        });
    }

    if let Some(extensions_required) = json
        .get_mut("extensionsRequired")
        .and_then(Value::as_array_mut)
    {
        extensions_required.retain(|entry| {
            entry
                .as_str()
                .map(|name| !name.to_ascii_lowercase().contains("vrm"))
                .unwrap_or(true)
        });
    }

    remove_key_recursively(json, "extras");
}

/// Remove unsupported animation and morph-target features.
pub(super) fn remove_unsupported_features(json: &mut Value) {
    json.as_object_mut().map(|root| {
        root.remove("animations");
    });

    if let Some(meshes) = json.get_mut("meshes").and_then(Value::as_array_mut) {
        for mesh in meshes {
            if let Some(primitives) = mesh.get_mut("primitives").and_then(Value::as_array_mut) {
                for primitive in primitives {
                    primitive.as_object_mut().map(|obj| {
                        obj.remove("targets");
                    });
                }
            }
        }
    }
}

/// Remove a key recursively from an arbitrary JSON tree.
fn remove_key_recursively(value: &mut Value, target_key: &str) {
    match value {
        Value::Object(map) => {
            map.remove(target_key);
            for (_, v) in map.iter_mut() {
                remove_key_recursively(v, target_key);
            }
        }
        Value::Array(array) => {
            for v in array.iter_mut() {
                remove_key_recursively(v, target_key);
            }
        }
        _ => {}
    }
}
