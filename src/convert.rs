use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result, bail};
use gltf::{Document, Semantic, binary::Glb, import};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::texture::ResizeInterpolation;

const REQUIRED_BONES: [&str; 17] = [
    "hips",
    "spine",
    "chest",
    "neck",
    "head",
    "leftUpperArm",
    "leftLowerArm",
    "leftHand",
    "rightUpperArm",
    "rightLowerArm",
    "rightHand",
    "leftUpperLeg",
    "leftLowerLeg",
    "leftFoot",
    "rightUpperLeg",
    "rightLowerLeg",
    "rightFoot",
];

const BONE_MAP: [(&str, &str); 17] = [
    ("hips", "mPelvis"),
    ("spine", "mTorso"),
    ("chest", "mChest"),
    ("neck", "mNeck"),
    ("head", "mHead"),
    ("leftUpperArm", "mShoulderLeft"),
    ("leftLowerArm", "mElbowLeft"),
    ("leftHand", "mWristLeft"),
    ("rightUpperArm", "mShoulderRight"),
    ("rightLowerArm", "mElbowRight"),
    ("rightHand", "mWristRight"),
    ("leftUpperLeg", "mHipLeft"),
    ("leftLowerLeg", "mKneeLeft"),
    ("leftFoot", "mAnkleLeft"),
    ("rightUpperLeg", "mHipRight"),
    ("rightLowerLeg", "mKneeRight"),
    ("rightFoot", "mAnkleRight"),
];

#[derive(Debug, Clone, Copy)]
pub struct ConvertOptions {
    pub target_height_cm: f32,
    pub manual_scale: f32,
    pub texture_auto_resize: bool,
    pub texture_resize_method: ResizeInterpolation,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            target_height_cm: 200.0,
            manual_scale: 1.0,
            texture_auto_resize: true,
            texture_resize_method: ResizeInterpolation::Bilinear,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureInfo {
    pub index: usize,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadFeeEstimate {
    pub before_linden_dollar: u32,
    pub after_resize_linden_dollar: u32,
    pub reduction_percent: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub model_name: String,
    pub author: Option<String>,
    pub estimated_height_cm: f32,
    pub bone_count: usize,
    pub mesh_count: usize,
    pub total_vertices: usize,
    pub total_polygons: usize,
    pub mapped_bones: Vec<(String, String)>,
    pub missing_required_bones: Vec<String>,
    pub texture_infos: Vec<TextureInfo>,
    pub fee_estimate: UploadFeeEstimate,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversionReport {
    pub model_name: String,
    pub author: Option<String>,
    pub estimated_height_cm: f32,
    pub target_height_cm: f32,
    pub computed_scale_factor: f32,
    pub bone_count: usize,
    pub mesh_count: usize,
    pub total_vertices: usize,
    pub total_polygons: usize,
    pub mapped_bones: Vec<(String, String)>,
    pub texture_count: usize,
    pub texture_over_1024_count: usize,
    pub fee_estimate: UploadFeeEstimate,
    pub issues: Vec<ValidationIssue>,
}

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

pub fn analyze_vrm(input_path: &Path, options: ConvertOptions) -> Result<AnalysisReport> {
    let input_bytes = fs::read(input_path)
        .with_context(|| format!("failed to read input file: {}", input_path.display()))?;
    let input_glb = Glb::from_slice(&input_bytes).context("input VRM is not a GLB container")?;
    let input_json: Value = serde_json::from_slice(input_glb.json.as_ref())
        .context("failed to parse glTF JSON chunk from VRM")?;

    let (document, buffers, images) = import(input_path)
        .with_context(|| format!("failed to read VRM/glTF: {}", input_path.display()))?;

    let mut issues = Vec::<ValidationIssue>::new();

    if let Err(err) = validate_vroid_model(&input_json) {
        issues.push(ValidationIssue {
            severity: Severity::Error,
            code: "UNSUPPORTED_SOURCE".to_string(),
            message: err.to_string(),
        });
    }

    let node_names = collect_node_names(&document);
    let parent_map = collect_parent_map(&document);
    let missing_required_bones = collect_missing_required_bones(&node_names);

    for missing in &missing_required_bones {
        issues.push(ValidationIssue {
            severity: Severity::Error,
            code: "MISSING_REQUIRED_BONE".to_string(),
            message: format!("[ERROR] 必須ボーン {} が見つかりません", missing),
        });
    }

    issues.extend(validate_hierarchy(&node_names, &parent_map));

    let (total_vertices, total_polygons, mut geometry_issues) = collect_mesh_statistics(&document);
    issues.append(&mut geometry_issues);

    let texture_infos: Vec<TextureInfo> = images
        .iter()
        .enumerate()
        .map(|(index, image)| TextureInfo {
            index,
            width: image.width,
            height: image.height,
        })
        .collect();

    let oversized_count = texture_infos
        .iter()
        .filter(|texture| texture.width > 1024 || texture.height > 1024)
        .count();

    if oversized_count > 0 {
        issues.push(ValidationIssue {
            severity: if options.texture_auto_resize {
                Severity::Info
            } else {
                Severity::Warning
            },
            code: "TEXTURE_OVERSIZE".to_string(),
            message: if options.texture_auto_resize {
                format!(
                    "⚠️ テクスチャサイズ超過 {} 枚を検出。エクスポート時に1024px上限へ縮小予定です",
                    oversized_count
                )
            } else {
                format!(
                    "⚠️ テクスチャサイズ超過 {} 枚を検出。Second Lifeアップロード費用増加の可能性があります",
                    oversized_count
                )
            },
        });
    }

    let fee_estimate = estimate_texture_fee(&texture_infos);

    let estimated_height_cm = estimate_height_cm(&document, &buffers).unwrap_or(170.0);

    Ok(AnalysisReport {
        model_name: extract_model_name(&input_json)
            .unwrap_or_else(|| input_path.to_string_lossy().to_string()),
        author: extract_author(&input_json),
        estimated_height_cm,
        bone_count: node_names.len(),
        mesh_count: document.meshes().count(),
        total_vertices,
        total_polygons,
        mapped_bones: collect_mapped_bones(&node_names),
        missing_required_bones,
        texture_infos,
        fee_estimate,
        issues,
    })
}

pub fn convert_vrm_to_gdb(
    input_path: &Path,
    output_path: &Path,
    options: ConvertOptions,
) -> Result<ConversionReport> {
    let analysis = analyze_vrm(input_path, options)?;

    if !analysis.missing_required_bones.is_empty() {
        bail!(
            "[ERROR] 必須ボーン不足: {}",
            analysis.missing_required_bones.join(", ")
        );
    }

    if analysis
        .issues
        .iter()
        .any(|issue| issue.severity == Severity::Error)
    {
        let message = analysis
            .issues
            .iter()
            .filter(|issue| issue.severity == Severity::Error)
            .map(|issue| issue.message.clone())
            .collect::<Vec<String>>()
            .join(" / ");
        bail!(message);
    }

    let computed_scale_factor = if analysis.estimated_height_cm > 0.0 {
        (options.target_height_cm / analysis.estimated_height_cm) * options.manual_scale
    } else {
        options.manual_scale
    };

    transform_and_write_glb(input_path, output_path, computed_scale_factor)?;

    let texture_over_1024_count = analysis
        .texture_infos
        .iter()
        .filter(|image| image.width > 1024 || image.height > 1024)
        .count();

    Ok(ConversionReport {
        model_name: analysis.model_name,
        author: analysis.author,
        estimated_height_cm: analysis.estimated_height_cm,
        target_height_cm: options.target_height_cm,
        computed_scale_factor,
        bone_count: analysis.bone_count,
        mesh_count: analysis.mesh_count,
        total_vertices: analysis.total_vertices,
        total_polygons: analysis.total_polygons,
        mapped_bones: analysis.mapped_bones,
        texture_count: analysis.texture_infos.len(),
        texture_over_1024_count,
        fee_estimate: analysis.fee_estimate,
        issues: analysis.issues,
    })
}

fn validate_vroid_model(json: &Value) -> Result<()> {
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

    bail!("[ERROR] VRoid Studio標準のVRMのみサポートしています")
}

fn collect_node_names(document: &Document) -> HashSet<String> {
    document
        .nodes()
        .filter_map(|node| node.name().map(ToOwned::to_owned))
        .collect()
}

fn collect_parent_map(document: &Document) -> HashMap<String, String> {
    let mut parent_map = HashMap::new();
    for parent in document.nodes() {
        let Some(parent_name) = parent.name() else {
            continue;
        };

        for child in parent.children() {
            if let Some(child_name) = child.name() {
                parent_map.insert(child_name.to_string(), parent_name.to_string());
            }
        }
    }

    parent_map
}

fn collect_missing_required_bones(node_names: &HashSet<String>) -> Vec<String> {
    REQUIRED_BONES
        .iter()
        .filter(|bone_name| !node_names.contains(**bone_name))
        .map(|bone_name| bone_name.to_string())
        .collect()
}

fn validate_hierarchy(
    node_names: &HashSet<String>,
    parent_map: &HashMap<String, String>,
) -> Vec<ValidationIssue> {
    REQUIRED_PARENT_RELATIONS
        .iter()
        .filter_map(|(parent, child)| {
            if !node_names.contains(*parent) || !node_names.contains(*child) {
                return None;
            }

            let Some(actual_parent) = parent_map.get(*child) else {
                return Some(ValidationIssue {
                    severity: Severity::Error,
                    code: "INVALID_BONE_HIERARCHY".to_string(),
                    message: format!("[ERROR] 非標準的なボーン階層です: {} の親が未設定", child),
                });
            };

            if actual_parent != parent {
                return Some(ValidationIssue {
                    severity: Severity::Error,
                    code: "INVALID_BONE_HIERARCHY".to_string(),
                    message: format!(
                        "[ERROR] 非標準的なボーン階層です: {} の親が {} ではなく {} です",
                        child, parent, actual_parent
                    ),
                });
            }

            None
        })
        .collect()
}

fn collect_mesh_statistics(document: &Document) -> (usize, usize, Vec<ValidationIssue>) {
    let mut total_vertices = 0usize;
    let mut total_polygons = 0usize;
    let mut issues = Vec::<ValidationIssue>::new();

    for mesh in document.meshes() {
        let mesh_name = mesh.name().unwrap_or("unnamed_mesh");
        for (primitive_index, primitive) in mesh.primitives().enumerate() {
            let vertex_count = primitive
                .get(&Semantic::Positions)
                .map(|accessor| accessor.count())
                .unwrap_or(0);

            total_vertices += vertex_count;

            if vertex_count > 65_535 {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "VERTEX_LIMIT_EXCEEDED".to_string(),
                    message: format!(
                        "⛔ 頂点数オーバー（メッシュ: {}, primitive: {}, 現在: {} / 上限: 65535）",
                        mesh_name, primitive_index, vertex_count
                    ),
                });
            }

            let index_count = primitive
                .indices()
                .map(|indices| indices.count())
                .unwrap_or(0);

            if index_count > 0 {
                total_polygons += index_count / 3;
            } else {
                total_polygons += vertex_count / 3;
            }
        }
    }

    (total_vertices, total_polygons, issues)
}

fn estimate_texture_fee(texture_infos: &[TextureInfo]) -> UploadFeeEstimate {
    let before = texture_infos
        .iter()
        .map(|texture| fee_per_texture(texture.width, texture.height))
        .sum::<u32>();

    let after = texture_infos
        .iter()
        .map(|texture| {
            let clamped_width = texture.width.min(1024);
            let clamped_height = texture.height.min(1024);
            fee_per_texture(clamped_width, clamped_height)
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

fn fee_per_texture(width: u32, height: u32) -> u32 {
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

fn collect_mapped_bones(node_names: &HashSet<String>) -> Vec<(String, String)> {
    BONE_MAP
        .iter()
        .filter(|(source, _)| node_names.contains(*source))
        .map(|(source, target)| (source.to_string(), target.to_string()))
        .collect()
}

fn estimate_height_cm(document: &Document, buffers: &[gltf::buffer::Data]) -> Option<f32> {
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|b| &b.0[..]));
            if let Some(positions) = reader.read_positions() {
                for p in positions {
                    min_y = min_y.min(p[1]);
                    max_y = max_y.max(p[1]);
                }
            }
        }
    }

    if min_y.is_finite() && max_y.is_finite() {
        Some((max_y - min_y).abs() * 100.0)
    } else {
        None
    }
}

fn transform_and_write_glb(input_path: &Path, output_path: &Path, scale_factor: f32) -> Result<()> {
    let bytes = fs::read(input_path)
        .with_context(|| format!("failed to read input file: {}", input_path.display()))?;
    let glb = Glb::from_slice(&bytes).context("input VRM is not a GLB container")?;

    let mut json: Value = serde_json::from_slice(glb.json.as_ref())
        .context("failed to parse glTF JSON chunk from VRM")?;

    rename_bones(&mut json);
    remove_vrm_extensions_and_extras(&mut json);
    remove_unsupported_features(&mut json);
    apply_uniform_scale_to_scene_roots(&mut json, scale_factor);

    let json_bytes =
        serde_json::to_vec(&json).context("failed to serialize transformed glTF JSON")?;

    let transformed = Glb {
        header: glb.header,
        json: Cow::Owned(json_bytes),
        bin: glb.bin,
    };

    let mut out = Vec::new();
    transformed
        .to_writer(&mut out)
        .context("failed to write output GLB")?;

    fs::write(output_path, out)
        .with_context(|| format!("failed to write output: {}", output_path.display()))?;

    Ok(())
}

fn rename_bones(json: &mut Value) {
    let map: HashMap<&str, &str> = BONE_MAP.into_iter().collect();

    if let Some(nodes) = json.get_mut("nodes").and_then(Value::as_array_mut) {
        for node in nodes {
            if let Some(current_name) = node.get("name").and_then(Value::as_str) {
                if let Some(new_name) = map.get(current_name) {
                    node["name"] = Value::String((*new_name).to_string());
                }
            }
        }
    }
}

fn remove_vrm_extensions_and_extras(json: &mut Value) {
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

fn remove_unsupported_features(json: &mut Value) {
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

fn apply_uniform_scale_to_scene_roots(json: &mut Value, scale_factor: f32) {
    let root_node_indices = json
        .get("scenes")
        .and_then(Value::as_array)
        .and_then(|scenes| scenes.first())
        .and_then(|scene| scene.get("nodes"))
        .and_then(Value::as_array)
        .cloned();

    let Some(root_node_indices) = root_node_indices else {
        return;
    };

    if let Some(nodes) = json.get_mut("nodes").and_then(Value::as_array_mut) {
        for node_index in root_node_indices
            .into_iter()
            .filter_map(|index| index.as_u64().map(|n| n as usize))
        {
            if let Some(node) = nodes.get_mut(node_index) {
                let existing = node
                    .get("scale")
                    .and_then(Value::as_array)
                    .and_then(|values| {
                        if values.len() == 3 {
                            Some(Vector3::new(
                                values[0].as_f64().unwrap_or(1.0) as f32,
                                values[1].as_f64().unwrap_or(1.0) as f32,
                                values[2].as_f64().unwrap_or(1.0) as f32,
                            ))
                        } else {
                            None
                        }
                    })
                    .unwrap_or(Vector3::new(1.0, 1.0, 1.0));

                let result = existing * scale_factor;
                node["scale"] = serde_json::json!([result.x, result.y, result.z]);
            }
        }
    }
}

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

fn extract_model_name(json: &Value) -> Option<String> {
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

fn extract_author(json: &Value) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_texture_sizes_when_estimating_fee_then_reduction_is_computed() {
        let textures = vec![
            TextureInfo {
                index: 0,
                width: 2048,
                height: 2048,
            },
            TextureInfo {
                index: 1,
                width: 1024,
                height: 1024,
            },
        ];

        let estimate = estimate_texture_fee(&textures);
        assert!(estimate.before_linden_dollar > estimate.after_resize_linden_dollar);
        assert!(estimate.reduction_percent > 0);
    }

    #[test]
    fn given_required_hierarchy_when_parent_mismatch_then_error_is_reported() {
        let node_names = ["hips", "spine", "chest"]
            .iter()
            .map(|name| name.to_string())
            .collect::<HashSet<String>>();

        let mut parent_map = HashMap::new();
        parent_map.insert("spine".to_string(), "hips".to_string());
        parent_map.insert("chest".to_string(), "hips".to_string());

        let issues = validate_hierarchy(&node_names, &parent_map);
        assert!(
            issues
                .iter()
                .any(|issue| issue.code == "INVALID_BONE_HIERARCHY")
        );
    }
}
