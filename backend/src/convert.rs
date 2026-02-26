use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fs,
    io::Cursor,
    path::Path,
};

use anyhow::{Context, Result, bail};
use gltf::{Document, Semantic, binary::Glb, import};
use image::ImageFormat;
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::texture::{ResizeInterpolation, resize_texture_to_max};

/// Required humanoid source bone names expected in VRoid/VRM input.
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

/// Core VRM-to-Second Life bone mapping table.
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

/// Conversion options shared by CLI and Tauri IPC entry points.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ConvertOptions {
    /// Target avatar height in centimeters.
    pub target_height_cm: f32,
    /// Additional manual scale multiplier.
    pub manual_scale: f32,
    /// Enables automatic texture downscaling checks/policy.
    pub texture_auto_resize: bool,
    /// Interpolation method used for texture resize operations.
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

/// Severity level used by validation issues.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A single validation issue produced during analysis/conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

/// Texture metadata used in validation and upload cost estimation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureInfo {
    pub index: usize,
    pub width: u32,
    pub height: u32,
}

/// Lightweight upload fee estimate before and after resize policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadFeeEstimate {
    pub before_linden_dollar: u32,
    pub after_resize_linden_dollar: u32,
    pub reduction_percent: u32,
}

/// Analysis-only report generated without writing output files.
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

/// Full conversion report returned after export.
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
    pub output_texture_infos: Vec<TextureInfo>,
    pub output_texture_over_1024_count: usize,
    pub fee_estimate: UploadFeeEstimate,
    pub issues: Vec<ValidationIssue>,
}

/// Required parent-child relationships used for hierarchy validation.
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

/// Analyze a VRM/GLB file and return validation + diagnostic information.
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

    let humanoid_bone_nodes = extract_humanoid_bone_nodes(&input_json);
    let node_names = collect_node_names(&document);
    let parent_index_map = collect_parent_index_map(&document);
    let missing_required_bones = collect_missing_required_bones(&humanoid_bone_nodes);

    for missing in &missing_required_bones {
        issues.push(ValidationIssue {
            severity: Severity::Error,
            code: "MISSING_REQUIRED_BONE".to_string(),
            message: format!("[ERROR] Required bone '{}' was not found", missing),
        });
    }

    issues.extend(validate_hierarchy(&humanoid_bone_nodes, &parent_index_map));

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

    let medium_oversized_count = texture_infos
        .iter()
        .filter(|texture| {
            let max_dim = texture.width.max(texture.height);
            max_dim > 1024 && max_dim <= 2048
        })
        .count();
    let large_oversized_count = texture_infos
        .iter()
        .filter(|texture| texture.width.max(texture.height) > 2048)
        .count();

    if medium_oversized_count > 0 {
        issues.push(ValidationIssue {
            severity: Severity::Warning,
            code: "TEXTURE_OVERSIZE_1024_2048".to_string(),
            message: format!(
                "⚠️ Detected {} texture(s) with max edge between 1025 and 2048. Enable the 1024px resize option if you want to downscale them",
                medium_oversized_count
            ),
        });
    }

    if large_oversized_count > 0 {
        issues.push(ValidationIssue {
            severity: Severity::Warning,
            code: "TEXTURE_OVERSIZE_OVER_2048".to_string(),
            message: if options.texture_auto_resize {
                format!(
                    "⚠️ Detected {} texture(s) larger than 2048. They will be resized to a 1024px max on export",
                    large_oversized_count
                )
            } else {
                format!(
                    "⚠️ Detected {} texture(s) larger than 2048. They will be resized to a 2048px max on export",
                    large_oversized_count
                )
            },
        });
    }

    let fee_estimate = estimate_texture_fee(&texture_infos, options.texture_auto_resize);

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
        mapped_bones: collect_mapped_bones(&humanoid_bone_nodes),
        missing_required_bones,
        texture_infos,
        fee_estimate,
        issues,
    })
}

/// Convert a VRM file to Second Life-oriented `.gdb` output.
pub fn convert_vrm_to_gdb(
    input_path: &Path,
    output_path: &Path,
    options: ConvertOptions,
) -> Result<ConversionReport> {
    let analysis = analyze_vrm(input_path, options)?;
    let input_json = parse_glb_json(input_path)?;
    let humanoid_bone_nodes = extract_humanoid_bone_nodes(&input_json);

    if !analysis.missing_required_bones.is_empty() {
        bail!(
            "[ERROR] Missing required bones: {}",
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

    transform_and_write_glb(
        input_path,
        output_path,
        computed_scale_factor,
        &humanoid_bone_nodes,
        options.texture_auto_resize,
        options.texture_resize_method,
    )?;

    let output_texture_infos = collect_output_texture_infos(output_path)?;
    let output_texture_over_1024_count = output_texture_infos
        .iter()
        .filter(|image| image.width > 1024 || image.height > 1024)
        .count();

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
        output_texture_infos,
        output_texture_over_1024_count,
        fee_estimate: analysis.fee_estimate,
        issues: analysis.issues,
    })
}

/// Collect texture dimensions from an exported GLB/GDB output file.
fn collect_output_texture_infos(output_path: &Path) -> Result<Vec<TextureInfo>> {
    let (_, _, images) = import(output_path)
        .with_context(|| format!("failed to read output VRM/glTF: {}", output_path.display()))?;

    Ok(images
        .iter()
        .enumerate()
        .map(|(index, image)| TextureInfo {
            index,
            width: image.width,
            height: image.height,
        })
        .collect())
}

/// Parse and return the JSON chunk from a GLB/VRM file.
fn parse_glb_json(input_path: &Path) -> Result<Value> {
    let input_bytes = fs::read(input_path)
        .with_context(|| format!("failed to read input file: {}", input_path.display()))?;
    let input_glb = Glb::from_slice(&input_bytes).context("input VRM is not a GLB container")?;
    serde_json::from_slice(input_glb.json.as_ref())
        .context("failed to parse glTF JSON chunk from VRM")
}

/// Validate that the source appears to be a supported VRoid/VRM model.
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

    bail!("[ERROR] Only standard VRoid Studio VRM files are supported")
}

/// Collect all named node identifiers from the source document.
fn collect_node_names(document: &Document) -> HashSet<String> {
    document
        .nodes()
        .filter_map(|node| node.name().map(ToOwned::to_owned))
        .collect()
}

/// Build a child->parent node-name map for hierarchy validation.
fn collect_parent_index_map(document: &Document) -> HashMap<usize, usize> {
    let mut parent_map = HashMap::new();
    for parent in document.nodes() {
        for child in parent.children() {
            parent_map.insert(child.index(), parent.index());
        }
    }

    parent_map
}

/// Return missing required bones from the source node-name set.
fn collect_missing_required_bones(humanoid_bone_nodes: &HashMap<String, usize>) -> Vec<String> {
    REQUIRED_BONES
        .iter()
        .filter(|bone_name| !humanoid_bone_nodes.contains_key(**bone_name))
        .map(|bone_name| bone_name.to_string())
        .collect()
}

/// Validate required humanoid hierarchy relationships.
fn validate_hierarchy(
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

/// Collect total mesh statistics and hard-limit validation issues.
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
                        "⛔ Vertex limit exceeded (mesh: {}, primitive: {}, current: {} / limit: 65535)",
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

/// Estimate texture upload fees before/after resize policy.
fn estimate_texture_fee(
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
fn projected_texture_size(width: u32, height: u32, auto_resize_to_1024: bool) -> (u32, u32) {
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

/// Return mapped source->target bone pairs present in the input model.
fn collect_mapped_bones(humanoid_bone_nodes: &HashMap<String, usize>) -> Vec<(String, String)> {
    BONE_MAP
        .iter()
        .filter(|(source, _)| humanoid_bone_nodes.contains_key(*source))
        .map(|(source, target)| (source.to_string(), target.to_string()))
        .collect()
}

/// Estimate avatar height in centimeters from mesh Y extents.
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

/// Apply transforms/cleanup and write the final GLB/GDB output.
fn transform_and_write_glb(
    input_path: &Path,
    output_path: &Path,
    scale_factor: f32,
    humanoid_bone_nodes: &HashMap<String, usize>,
    texture_auto_resize: bool,
    texture_resize_method: ResizeInterpolation,
) -> Result<()> {
    let bytes = fs::read(input_path)
        .with_context(|| format!("failed to read input file: {}", input_path.display()))?;
    let glb = Glb::from_slice(&bytes).context("input VRM is not a GLB container")?;

    let mut json: Value = serde_json::from_slice(glb.json.as_ref())
        .context("failed to parse glTF JSON chunk from VRM")?;
    let had_bin = glb.bin.is_some();
    let mut bin = glb.bin.map(|chunk| chunk.into_owned()).unwrap_or_default();

    rename_bones(&mut json, humanoid_bone_nodes);
    remove_vrm_extensions_and_extras(&mut json);
    remove_unsupported_features(&mut json);
    apply_uniform_scale_to_scene_roots(&mut json, scale_factor);

    apply_texture_resize_to_embedded_images(
        &mut json,
        &mut bin,
        texture_auto_resize,
        texture_resize_method,
    )?;

    let json_bytes =
        serde_json::to_vec(&json).context("failed to serialize transformed glTF JSON")?;

    let transformed = Glb {
        header: glb.header,
        json: Cow::Owned(json_bytes),
        bin: if had_bin || !bin.is_empty() {
            Some(Cow::Owned(bin))
        } else {
            None
        },
    };

    let mut out = Vec::new();
    transformed
        .to_writer(&mut out)
        .context("failed to write output GLB")?;

    fs::write(output_path, out)
        .with_context(|| format!("failed to write output: {}", output_path.display()))?;

    Ok(())
}

/// Resize embedded image buffer views when textures exceed 1024x1024.
fn apply_texture_resize_to_embedded_images(
    json: &mut Value,
    bin: &mut Vec<u8>,
    auto_resize_to_1024: bool,
    interpolation: ResizeInterpolation,
) -> Result<()> {
    let Some(buffer_views) = json.get("bufferViews").and_then(Value::as_array) else {
        return Ok(());
    };

    let mut segments = Vec::<Vec<u8>>::with_capacity(buffer_views.len());
    for view in buffer_views {
        let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let length = view.get("byteLength").and_then(Value::as_u64).unwrap_or(0) as usize;
        let end = offset.saturating_add(length);
        if end <= bin.len() {
            segments.push(bin[offset..end].to_vec());
        } else {
            segments.push(Vec::new());
        }
    }

    if let Some(images) = json.get_mut("images").and_then(Value::as_array_mut) {
        for image in images {
            let Some(view_index) = image
                .get("bufferView")
                .and_then(Value::as_u64)
                .map(|index| index as usize)
            else {
                continue;
            };

            let Some(image_bytes) = segments.get_mut(view_index) else {
                continue;
            };

            let mime_type = image
                .get("mimeType")
                .and_then(Value::as_str)
                .unwrap_or("image/png");
            let image_format = match mime_type {
                "image/png" => Some(ImageFormat::Png),
                "image/jpeg" | "image/jpg" => Some(ImageFormat::Jpeg),
                _ => None,
            };

            let Some(image_format) = image_format else {
                continue;
            };

            let decoded = image::load_from_memory(image_bytes)
                .with_context(|| format!("failed to decode embedded texture view {view_index}"))?;

            let max_dim = decoded.width().max(decoded.height());
            let target_max = if max_dim <= 1024 {
                1024
            } else if max_dim <= 2048 {
                if auto_resize_to_1024 { 1024 } else { max_dim }
            } else if auto_resize_to_1024 {
                1024
            } else {
                2048
            };

            let resized = if max_dim > target_max {
                resize_texture_to_max(&decoded, target_max, target_max, interpolation)
            } else {
                decoded
            };
            let mut encoded = Vec::<u8>::new();
            resized
                .write_to(&mut Cursor::new(&mut encoded), image_format)
                .with_context(|| {
                    format!(
                        "failed to encode resized embedded texture view {} as {}",
                        view_index, mime_type
                    )
                })?;

            *image_bytes = encoded;
            image["mimeType"] = Value::String(match image_format {
                ImageFormat::Png => "image/png".to_string(),
                ImageFormat::Jpeg => "image/jpeg".to_string(),
                _ => mime_type.to_string(),
            });
        }
    }

    let mut rebuilt = Vec::<u8>::new();
    let mut offsets = Vec::<usize>::with_capacity(segments.len());
    for mut chunk in segments {
        while rebuilt.len() % 4 != 0 {
            rebuilt.push(0);
        }
        offsets.push(rebuilt.len());
        rebuilt.append(&mut chunk);
    }

    if let Some(buffer_views_mut) = json.get_mut("bufferViews").and_then(Value::as_array_mut) {
        for (index, view) in buffer_views_mut.iter_mut().enumerate() {
            if let Some(offset) = offsets.get(index).copied() {
                let length = rebuilt.len().saturating_sub(offset);
                let next_offset = offsets.get(index + 1).copied().unwrap_or(rebuilt.len());
                let byte_length = next_offset.saturating_sub(offset);
                view["byteOffset"] = Value::from(offset as u64);
                view["byteLength"] = Value::from(byte_length as u64);
                let _ = length;
            }
        }
    }

    if let Some(buffers) = json.get_mut("buffers").and_then(Value::as_array_mut) {
        if let Some(first_buffer) = buffers.first_mut() {
            first_buffer["byteLength"] = Value::from(rebuilt.len() as u64);
        }
    }

    *bin = rebuilt;
    Ok(())
}

/// Rename known bones according to the mapping table.
fn rename_bones(json: &mut Value, humanoid_bone_nodes: &HashMap<String, usize>) {
    if let Some(nodes) = json.get_mut("nodes").and_then(Value::as_array_mut) {
        for (source, target) in BONE_MAP {
            if let Some(node_index) = humanoid_bone_nodes.get(source).copied() {
                if let Some(node) = nodes.get_mut(node_index) {
                    node["name"] = Value::String(target.to_string());
                }
            }
        }
    }
}

/// Extract humanoid-bone semantic to node-index mapping from VRM extensions.
fn extract_humanoid_bone_nodes(json: &Value) -> HashMap<String, usize> {
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

/// Remove VRM-specific extensions and recursive extras fields.
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

/// Remove unsupported animation and morph-target features.
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

/// Apply uniform scale to scene root nodes.
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

/// Extract model name from VRM metadata or asset generator.
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

/// Extract author/copyright metadata from VRM or asset section.
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

        let estimate = estimate_texture_fee(&textures, true);
        assert!(estimate.before_linden_dollar > estimate.after_resize_linden_dollar);
        assert!(estimate.reduction_percent > 0);
    }

    #[test]
    fn given_large_texture_when_1024_resize_disabled_then_policy_caps_at_2048() {
        let (width, height) = projected_texture_size(4096, 2048, false);
        assert_eq!(width, 2048);
        assert_eq!(height, 1024);
    }

    #[test]
    fn given_mid_texture_when_1024_resize_disabled_then_size_is_kept() {
        let (width, height) = projected_texture_size(1800, 900, false);
        assert_eq!(width, 1800);
        assert_eq!(height, 900);
    }

    #[test]
    fn given_required_hierarchy_when_parent_mismatch_then_error_is_reported() {
        let humanoid_bone_nodes = ["hips", "spine", "chest"]
            .iter()
            .enumerate()
            .map(|(index, name)| (name.to_string(), index))
            .collect::<HashMap<String, usize>>();

        let mut parent_map = HashMap::<usize, usize>::new();
        parent_map.insert(1, 0);
        parent_map.insert(2, 0);

        let issues = validate_hierarchy(&humanoid_bone_nodes, &parent_map);
        assert!(
            issues
                .iter()
                .any(|issue| issue.code == "INVALID_BONE_HIERARCHY")
        );
    }

    #[test]
    fn given_vrmc_humanoid_when_extracting_bones_then_required_bones_are_found() {
        let input_json = serde_json::json!({
            "extensions": {
                "VRMC_vrm": {
                    "humanoid": {
                        "humanBones": {
                            "hips": {"node": 1},
                            "spine": {"node": 2},
                            "chest": {"node": 3}
                        }
                    }
                }
            }
        });

        let mapping = extract_humanoid_bone_nodes(&input_json);
        assert_eq!(mapping.get("hips"), Some(&1usize));
        assert_eq!(mapping.get("spine"), Some(&2usize));
        assert_eq!(mapping.get("chest"), Some(&3usize));
    }
}
