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
use nalgebra::{Matrix3, Matrix4, Quaternion, Translation3, UnitQuaternion, Vector3};
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

/// Optional Bento extension mapping (eyes, jaw, fingers) from VRM humanoid
/// names to SL target bone names.
const BENTO_BONE_MAP: [(&str, &str); 33] = [
    ("leftEye", "mEyeLeft"),
    ("rightEye", "mEyeRight"),
    ("jaw", "mFaceJaw"),
    ("leftThumbProximal", "mHandThumb1Left"),
    ("leftThumbIntermediate", "mHandThumb2Left"),
    ("leftThumbDistal", "mHandThumb3Left"),
    ("leftIndexProximal", "mHandIndex1Left"),
    ("leftIndexIntermediate", "mHandIndex2Left"),
    ("leftIndexDistal", "mHandIndex3Left"),
    ("leftMiddleProximal", "mHandMiddle1Left"),
    ("leftMiddleIntermediate", "mHandMiddle2Left"),
    ("leftMiddleDistal", "mHandMiddle3Left"),
    ("leftRingProximal", "mHandRing1Left"),
    ("leftRingIntermediate", "mHandRing2Left"),
    ("leftRingDistal", "mHandRing3Left"),
    ("leftLittleProximal", "mHandPinky1Left"),
    ("leftLittleIntermediate", "mHandPinky2Left"),
    ("leftLittleDistal", "mHandPinky3Left"),
    ("rightThumbProximal", "mHandThumb1Right"),
    ("rightThumbIntermediate", "mHandThumb2Right"),
    ("rightThumbDistal", "mHandThumb3Right"),
    ("rightIndexProximal", "mHandIndex1Right"),
    ("rightIndexIntermediate", "mHandIndex2Right"),
    ("rightIndexDistal", "mHandIndex3Right"),
    ("rightMiddleProximal", "mHandMiddle1Right"),
    ("rightMiddleIntermediate", "mHandMiddle2Right"),
    ("rightMiddleDistal", "mHandMiddle3Right"),
    ("rightRingProximal", "mHandRing1Right"),
    ("rightRingIntermediate", "mHandRing2Right"),
    ("rightRingDistal", "mHandRing3Right"),
    ("rightLittleProximal", "mHandPinky1Right"),
    ("rightLittleIntermediate", "mHandPinky2Right"),
    ("rightLittleDistal", "mHandPinky3Right"),
];

/// Core hierarchy edges to reconstruct for SL-compatible humanoid skeleton.
const CORE_HIERARCHY_RELATIONS: [(&str, &str); 16] = [
    ("hips", "spine"),
    ("spine", "chest"),
    ("chest", "neck"),
    ("neck", "head"),
    ("chest", "leftUpperArm"),
    ("leftUpperArm", "leftLowerArm"),
    ("leftLowerArm", "leftHand"),
    ("chest", "rightUpperArm"),
    ("rightUpperArm", "rightLowerArm"),
    ("rightLowerArm", "rightHand"),
    ("hips", "leftUpperLeg"),
    ("leftUpperLeg", "leftLowerLeg"),
    ("leftLowerLeg", "leftFoot"),
    ("hips", "rightUpperLeg"),
    ("rightUpperLeg", "rightLowerLeg"),
    ("rightLowerLeg", "rightFoot"),
];

/// Optional hierarchy edges for Bento extension bones.
const BENTO_HIERARCHY_RELATIONS: [(&str, &str); 33] = [
    ("head", "leftEye"),
    ("head", "rightEye"),
    ("head", "jaw"),
    ("leftHand", "leftThumbProximal"),
    ("leftThumbProximal", "leftThumbIntermediate"),
    ("leftThumbIntermediate", "leftThumbDistal"),
    ("leftHand", "leftIndexProximal"),
    ("leftIndexProximal", "leftIndexIntermediate"),
    ("leftIndexIntermediate", "leftIndexDistal"),
    ("leftHand", "leftMiddleProximal"),
    ("leftMiddleProximal", "leftMiddleIntermediate"),
    ("leftMiddleIntermediate", "leftMiddleDistal"),
    ("leftHand", "leftRingProximal"),
    ("leftRingProximal", "leftRingIntermediate"),
    ("leftRingIntermediate", "leftRingDistal"),
    ("leftHand", "leftLittleProximal"),
    ("leftLittleProximal", "leftLittleIntermediate"),
    ("leftLittleIntermediate", "leftLittleDistal"),
    ("rightHand", "rightThumbProximal"),
    ("rightThumbProximal", "rightThumbIntermediate"),
    ("rightThumbIntermediate", "rightThumbDistal"),
    ("rightHand", "rightIndexProximal"),
    ("rightIndexProximal", "rightIndexIntermediate"),
    ("rightIndexIntermediate", "rightIndexDistal"),
    ("rightHand", "rightMiddleProximal"),
    ("rightMiddleProximal", "rightMiddleIntermediate"),
    ("rightMiddleIntermediate", "rightMiddleDistal"),
    ("rightHand", "rightRingProximal"),
    ("rightRingProximal", "rightRingIntermediate"),
    ("rightRingIntermediate", "rightRingDistal"),
    ("rightHand", "rightLittleProximal"),
    ("rightLittleProximal", "rightLittleIntermediate"),
    ("rightLittleIntermediate", "rightLittleDistal"),
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

/// Generate a reusable markdown checklist for final manual validation flow.
///
/// This checklist is intended for the final v0.8 step:
/// re-open converted output in any 3D modeling tool and verify before
/// uploading to Second Life.
pub fn write_final_validation_checklist(
    checklist_path: &Path,
    input_path: &Path,
    output_path: &Path,
    report: &ConversionReport,
) -> Result<()> {
    let mut content = String::new();
    content.push_str("# vrm2sl Final Validation Checklist\n\n");
    content.push_str("## Conversion Summary\n\n");
    content.push_str(&format!("- Input: `{}`\n", input_path.display()));
    content.push_str(&format!("- Output: `{}`\n", output_path.display()));
    content.push_str(&format!("- Model: `{}`\n", report.model_name));
    content.push_str(&format!(
        "- Estimated height: `{:.2} cm`\n",
        report.estimated_height_cm
    ));
    content.push_str(&format!(
        "- Target height: `{:.2} cm`\n",
        report.target_height_cm
    ));
    content.push_str(&format!(
        "- Computed scale: `{:.4}`\n",
        report.computed_scale_factor
    ));
    content.push_str(&format!(
        "- Meshes/Bones: `{}` / `{}`\n",
        report.mesh_count, report.bone_count
    ));
    content.push_str(&format!(
        "- Vertices/Polygons: `{}` / `{}`\n",
        report.total_vertices, report.total_polygons
    ));
    content.push_str(&format!(
        "- Texture fee estimate: `{}L$ -> {}L$`\n\n",
        report.fee_estimate.before_linden_dollar, report.fee_estimate.after_resize_linden_dollar
    ));

    content.push_str("## Validation Flow (Manual)\n\n");
    content.push_str("- [ ] Open the converted `.glb` file in any 3D modeling tool.\n");
    content.push_str("- [ ] Confirm armature loads without collapse/crash.\n");
    content.push_str("- [ ] Verify T-pose-like arm orientation (no severe A-pose residual).\n");
    content
        .push_str("- [ ] Verify core hierarchy shape (pelvis->torso->chest->neck->head, limbs).\n");
    content.push_str("- [ ] Verify eye/jaw/finger Bento bones exist when source contained them.\n");
    content.push_str("- [ ] Verify there is no obvious skin explosion or detached limbs.\n");
    content.push_str("- [ ] Upload to Second Life and confirm avatar deformation is acceptable.\n");
    content.push_str("- [ ] Confirm idle/walk behavior has no critical breakage in-world.\n\n");

    content.push_str("## Issues from Conversion\n\n");
    if report.issues.is_empty() {
        content.push_str("- None\n");
    } else {
        for issue in &report.issues {
            content.push_str(&format!("- [{:?}] {}\n", issue.severity, issue.message));
        }
    }

    fs::write(checklist_path, content).with_context(|| {
        format!(
            "failed to write validation checklist: {}",
            checklist_path.display()
        )
    })?;

    Ok(())
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
    issues.extend(validate_bone_conversion_preconditions(
        &input_json,
        &humanoid_bone_nodes,
    ));

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

/// Convert a VRM file to Second Life-oriented `.glb` output.
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

/// Collect texture dimensions from an exported GLB output file.
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
        .chain(BENTO_BONE_MAP.iter())
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

/// Apply transforms/cleanup and write the final GLB output.
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
    ensure_target_bones_exist_after_rename(&json, humanoid_bone_nodes)?;
    reconstruct_sl_core_hierarchy(&mut json, humanoid_bone_nodes);
    // Normalize all SL-mapped bone rotations to identity while preserving
    // their world-space positions.  Second Life reads bone bind positions from
    // the inverse-bind-matrix translations and applies its own (identity)
    // orientations in the SL skeleton; any non-identity rotation baked into
    // the node hierarchy will therefore cause incorrect deformation.
    normalize_sl_bone_rotations(&mut json, humanoid_bone_nodes);
    // Remap weights from unmapped VRM bones (e.g. upperChest, spring bones)
    // to their nearest mapped-SL ancestor so that only valid SL bones remain
    // in the skin joints list after optimization.
    remap_unmapped_bone_weights(&mut json, &mut bin, humanoid_bone_nodes);
    optimize_skinning_weights_and_joints(&mut json, &mut bin)?;
    // Set skin.skeleton to the hips (mPelvis) node so every importer agrees on
    // the skeleton root.
    set_skin_skeleton_to_pelvis(&mut json, humanoid_bone_nodes);
    // Bake the scale factor directly into geometry (node translations and mesh
    // vertex POSITION data) instead of setting a non-unit scale on root nodes.
    // This is the most universally compatible approach for SL: no root-scale
    // means no ambiguity about whether the renderer applies it before or after
    // skinning, and IBMs computed below are all in the final scaled space.
    bake_scale_into_geometry(&mut json, &mut bin, scale_factor)?;
    regenerate_inverse_bind_matrices(&mut json, &mut bin)?;
    remove_vrm_extensions_and_extras(&mut json);
    remove_unsupported_features(&mut json);

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
    let mut lengths = Vec::<usize>::with_capacity(segments.len());
    for mut chunk in segments {
        while rebuilt.len() % 4 != 0 {
            rebuilt.push(0);
        }
        offsets.push(rebuilt.len());
        lengths.push(chunk.len());
        rebuilt.append(&mut chunk);
    }

    if let Some(buffer_views_mut) = json.get_mut("bufferViews").and_then(Value::as_array_mut) {
        for (index, view) in buffer_views_mut.iter_mut().enumerate() {
            if let Some(offset) = offsets.get(index).copied() {
                let byte_length = lengths.get(index).copied().unwrap_or_default();
                view["byteOffset"] = Value::from(offset as u64);
                view["byteLength"] = Value::from(byte_length as u64);
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
        for (source, target) in BONE_MAP.iter().chain(BENTO_BONE_MAP.iter()) {
            if let Some(node_index) = humanoid_bone_nodes.get(*source).copied() {
                if let Some(node) = nodes.get_mut(node_index) {
                    node["name"] = Value::String(target.to_string());
                }
            }
        }
    }
}

/// Reconstruct core humanoid hierarchy toward SL-compatible parent-child links.
fn reconstruct_sl_core_hierarchy(json: &mut Value, humanoid_bone_nodes: &HashMap<String, usize>) {
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

fn set_node_local_matrix(node: &mut Value, matrix: &Matrix4<f32>) {
    let Some(object) = node.as_object_mut() else {
        return;
    };

    let translation = Vector3::new(matrix[(0, 3)], matrix[(1, 3)], matrix[(2, 3)]);

    let basis_x = Vector3::new(matrix[(0, 0)], matrix[(1, 0)], matrix[(2, 0)]);
    let basis_y = Vector3::new(matrix[(0, 1)], matrix[(1, 1)], matrix[(2, 1)]);
    let basis_z = Vector3::new(matrix[(0, 2)], matrix[(1, 2)], matrix[(2, 2)]);

    let mut scale_x = basis_x.norm();
    let scale_y = basis_y.norm();
    let scale_z = basis_z.norm();

    let mut rot_x = if scale_x > 1e-8 {
        basis_x / scale_x
    } else {
        Vector3::new(1.0, 0.0, 0.0)
    };
    let rot_y = if scale_y > 1e-8 {
        basis_y / scale_y
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    };
    let rot_z = if scale_z > 1e-8 {
        basis_z / scale_z
    } else {
        Vector3::new(0.0, 0.0, 1.0)
    };

    if rot_x.cross(&rot_y).dot(&rot_z) < 0.0 {
        scale_x = -scale_x;
        rot_x = -rot_x;
    }

    let rotation_matrix = Matrix3::from_columns(&[rot_x, rot_y, rot_z]);
    let rotation = UnitQuaternion::from_matrix(&rotation_matrix);

    object.remove("translation");
    object.remove("rotation");
    object.remove("scale");
    object.remove("matrix");
    object.insert(
        "translation".to_string(),
        serde_json::json!([translation.x, translation.y, translation.z]),
    );
    object.insert(
        "rotation".to_string(),
        serde_json::json!([
            rotation.coords.x,
            rotation.coords.y,
            rotation.coords.z,
            rotation.coords.w
        ]),
    );
    object.insert(
        "scale".to_string(),
        serde_json::json!([scale_x, scale_y, scale_z]),
    );
}

/// Validate that required source bones point to valid node indices before conversion.
fn validate_bone_conversion_preconditions(
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
                    severity: Severity::Error,
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
fn ensure_target_bones_exist_after_rename(
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

/// Collect all node names from glTF JSON nodes array.
fn collect_node_name_set_from_json(json: &Value) -> HashSet<String> {
    json.get("nodes")
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|node| {
                    node.get("name")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Normalize the local rotation of every SL-mapped bone to identity while
/// preserving the bone's world-space position.
///
/// Second Life reads joint bind-positions from the inverse-bind-matrix
/// (4th column) and then applies its **own** default (identity) orientations
/// when deforming the mesh. Any non-identity local rotation that was baked
/// into the glTF node hierarchy will therefore cause incorrect deformation
/// because the IBM accounts for the rotation but SL does not re-apply it.
///
/// The fix: process bones in topological (parent-before-child) order and preserve
/// each mapped bone's snapshot world-space position while zeroing local rotation.
/// This keeps bind positions stable and avoids introducing additional translation
/// offsets that can cause detached heads or twisted lower limbs in SL animations.
fn normalize_sl_bone_rotations(json: &mut Value, humanoid_bone_nodes: &HashMap<String, usize>) {
    // Set of all SL-mapped node indices.
    let sl_node_indices: HashSet<usize> = BONE_MAP
        .iter()
        .chain(BENTO_BONE_MAP.iter())
        .filter_map(|(vrm_name, _)| humanoid_bone_nodes.get(*vrm_name).copied())
        .collect();

    // Snapshot world matrices from the current (post-reconstruction) hierarchy.
    let node_locals: Vec<Matrix4<f32>> = json["nodes"]
        .as_array()
        .map(|nodes| nodes.iter().map(node_to_local_matrix).collect())
        .unwrap_or_default();
    let parent_map = collect_parent_index_map_from_json(json);
    let node_worlds_snapshot = compute_node_world_matrices(&node_locals, &parent_map);

    let node_count = json["nodes"].as_array().map(|a| a.len()).unwrap_or(0);

    // Compute topological order (parents before children) using BFS from roots.
    let mut topo_order: Vec<usize> = Vec::with_capacity(node_count);
    {
        let mut child_count = vec![0usize; node_count];
        for (&child, &parent) in &parent_map {
            let _ = parent;
            child_count[child] += 1; // just mark as having a parent
        }
        // roots = nodes with no parent
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

    // Effective world positions: start as snapshot, updated as each bone is processed.
    // This lets children use the corrected parent world position when computing
    // their own local translations.
    let mut effective_world_t: Vec<Vector3<f32>> = node_worlds_snapshot
        .iter()
        .map(|m| Vector3::new(m[(0, 3)], m[(1, 3)], m[(2, 3)]))
        .collect();
    // Pad if snapshot is shorter than node_count.
    while effective_world_t.len() < node_count {
        effective_world_t.push(Vector3::zeros());
    }

    for &node_idx in &topo_order {
        if !sl_node_indices.contains(&node_idx) {
            // Not an SL bone: keep effective world as snapshot (already set).
            continue;
        }

        // Snapshot world position of THIS bone (original bind pose target).
        let snapshot_t = match node_worlds_snapshot.get(node_idx) {
            Some(m) => Vector3::new(m[(0, 3)], m[(1, 3)], m[(2, 3)]),
            None => continue,
        };

        // Effective world position of the PARENT (already processed).
        let parent_effective_t = match parent_map.get(&node_idx) {
            Some(&parent_idx) => effective_world_t[parent_idx],
            None => Vector3::zeros(),
        };

        // Keep each mapped bone at its snapshot world position.
        let target_t = snapshot_t;

        // Local translation = target_world - parent_effective_world.
        let local_t = target_t - parent_effective_t;

        // Record this bone's effective world for its children.
        effective_world_t[node_idx] = parent_effective_t + local_t; // == target_t

        // Preserve the existing scale (default to [1,1,1]).
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

/// Remap vertex weights from non-SL (unmapped) VRM bones to their nearest SL-mapped
/// ancestor in the skeleton hierarchy. This ensures that only bones present in SL's
/// skeleton remain as active joints after `optimize_skinning_weights_and_joints` runs.
///
/// Unmapped bones include:
/// - `upperChest` (not in BONE_MAP; SL uses only chest/spine/neck chain)
/// - Spring/secondary bones (`J_Sec_*`) used for clothing/hair physics in VRM
///
/// The strategy: for each skin, identify which joint-slot indices refer to
/// unmapped node names (not starting with `m`), then scan every vertex and
/// for weight > 0 on an unmapped slot, transfer that weight to the nearest
/// ancestor that IS mapped (walk parent chain via `nodes[i].parent`).
fn remap_unmapped_bone_weights(
    json: &mut Value,
    bin: &mut [u8],
    humanoid_bone_nodes: &HashMap<String, usize>,
) {
    // Build a set of all SL-mapped node indices (after rename_bones these nodes
    // already have SL names, but we use indices for fast lookup).
    let sl_node_indices: HashSet<usize> = BONE_MAP
        .iter()
        .chain(BENTO_BONE_MAP.iter())
        .filter_map(|(vrm_name, _)| humanoid_bone_nodes.get(*vrm_name).copied())
        .collect();

    // Build parent map: node_index -> parent_index.
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

    // Helper: walk up the parent chain from `node_idx` until we find a node
    // in `sl_node_indices` or exhaust the ancestry.
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
        // Collect the joints list for this skin.
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

        // For each joint slot, decide if it needs to be remapped.
        // slot_remap[old_slot] = new_slot (may be the same if already SL).
        let mut slot_remap: Vec<usize> = (0..joints.len()).collect();
        let mut any_remap = false;
        for (slot, &node_idx) in joints.iter().enumerate() {
            if sl_node_indices.contains(&node_idx) {
                continue;
            }
            // Unmapped bone — find its nearest SL ancestor.
            if let Some(ancestor_node_idx) = find_sl_ancestor(node_idx) {
                if let Some(ancestor_slot) = joints.iter().position(|&j| j == ancestor_node_idx) {
                    slot_remap[slot] = ancestor_slot;
                    any_remap = true;
                }
                // If ancestor isn't in this skin's joint list yet, leave as-is;
                // optimize_skinning_weights_and_joints will compact it away if weight=0.
            }
        }

        if !any_remap {
            continue;
        }

        // Apply the slot remap to all primitive bindings for this skin.
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

                // Merge weights from unmapped slots into their remapped targets.
                let mut new_weights = [0.0f32; 4];
                let mut new_slots = [0u16; 4];
                // First pass: copy or accumulate into target slots.
                // We build a temporary per-slot accumulator indexed by slot index.
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

                // Pick the 4 highest-weight slots.
                let mut top4: Vec<(usize, f32)> = acc
                    .iter()
                    .enumerate()
                    .filter(|&(_, &w)| w > 1e-7)
                    .map(|(s, &w)| (s, w))
                    .collect();
                top4.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                top4.truncate(4);

                // Normalize.
                let weight_sum: f32 = top4.iter().map(|&(_, w)| w).sum();
                for lane in 0..4 {
                    if let Some(&(slot, w)) = top4.get(lane) {
                        new_slots[lane] = slot as u16;
                        new_weights[lane] = if weight_sum > 1e-7 {
                            w / weight_sum
                        } else {
                            0.0
                        };
                    } else {
                        new_slots[lane] = 0;
                        new_weights[lane] = 0.0;
                    }
                }

                // Write back.
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

/// Set `skin.skeleton` for every skin to the `mPelvis` node (hips in SL).
/// Without this, some importers (including the SL viewer) treat the skeleton
/// root as an identity node and produce incorrect world-space transforms when
/// evaluating inverse bind matrices.
///
/// For skins that don't contain `mPelvis` (e.g. facial rig), the skeleton is
/// set to the first joint in the skin's joint list, which is the safest fallback.
fn set_skin_skeleton_to_pelvis(json: &mut Value, humanoid_bone_nodes: &HashMap<String, usize>) {
    // Find the mPelvis node index (the renamed hips bone).
    // humanoid_bone_nodes keys are VRM bone names (e.g. "hips"), not SL names.
    let pelvis_index = humanoid_bone_nodes.get("hips").copied();

    let skins = match json["skins"].as_array_mut() {
        Some(s) => s,
        None => return,
    };

    for skin in skins.iter_mut() {
        let joints: Vec<usize> = skin["joints"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();

        if let Some(pelvis_idx) = pelvis_index {
            if joints.contains(&pelvis_idx) {
                skin["skeleton"] = Value::Number(pelvis_idx.into());
                continue;
            }
        }

        // Fallback: use the first joint as skeleton root.
        if let Some(&first) = joints.first() {
            skin["skeleton"] = Value::Number(first.into());
        }
    }
}

/// Rebuild inverse bind matrices from current node transforms and write them
/// back to the binary buffer for all skins that have writable MAT4 float accessors.
fn regenerate_inverse_bind_matrices(json: &mut Value, bin: &mut [u8]) -> Result<()> {
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

/// Collect child->parent node index mapping from glTF JSON.
fn collect_parent_index_map_from_json(json: &Value) -> HashMap<usize, usize> {
    let mut parent_map = HashMap::<usize, usize>::new();
    let Some(nodes) = json.get("nodes").and_then(Value::as_array) else {
        return parent_map;
    };

    for (parent_index, node) in nodes.iter().enumerate() {
        let Some(children) = node.get("children").and_then(Value::as_array) else {
            continue;
        };
        for child in children {
            if let Some(child_index) = child.as_u64().map(|value| value as usize) {
                parent_map.insert(child_index, parent_index);
            }
        }
    }

    parent_map
}

/// Build local transform matrix from a glTF node JSON object.
fn node_to_local_matrix(node: &Value) -> Matrix4<f32> {
    if let Some(matrix) = node.get("matrix").and_then(Value::as_array)
        && matrix.len() == 16
    {
        let mut values = [0.0f32; 16];
        for (index, value) in matrix.iter().enumerate() {
            values[index] = value.as_f64().unwrap_or(0.0) as f32;
        }
        return Matrix4::from_row_slice(&values).transpose();
    }

    let translation = node
        .get("translation")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .map(|values| {
            Vector3::new(
                values[0].as_f64().unwrap_or(0.0) as f32,
                values[1].as_f64().unwrap_or(0.0) as f32,
                values[2].as_f64().unwrap_or(0.0) as f32,
            )
        })
        .unwrap_or(Vector3::new(0.0, 0.0, 0.0));

    let rotation = node
        .get("rotation")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 4)
        .map(|values| {
            UnitQuaternion::from_quaternion(Quaternion::new(
                values[3].as_f64().unwrap_or(1.0) as f32,
                values[0].as_f64().unwrap_or(0.0) as f32,
                values[1].as_f64().unwrap_or(0.0) as f32,
                values[2].as_f64().unwrap_or(0.0) as f32,
            ))
        })
        .unwrap_or_else(UnitQuaternion::identity);

    let scale = node
        .get("scale")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .map(|values| {
            Vector3::new(
                values[0].as_f64().unwrap_or(1.0) as f32,
                values[1].as_f64().unwrap_or(1.0) as f32,
                values[2].as_f64().unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Vector3::new(1.0, 1.0, 1.0));

    let translation_matrix = Translation3::from(translation).to_homogeneous();
    let rotation_matrix = rotation.to_homogeneous();
    let scale_matrix = Matrix4::new_nonuniform_scaling(&scale);
    translation_matrix * rotation_matrix * scale_matrix
}

/// Compute world matrices from local transforms and parent links.
fn compute_node_world_matrices(
    local_matrices: &[Matrix4<f32>],
    parent_map: &HashMap<usize, usize>,
) -> Vec<Matrix4<f32>> {
    let mut worlds = vec![Matrix4::<f32>::identity(); local_matrices.len()];
    let mut resolved = vec![false; local_matrices.len()];

    for index in 0..local_matrices.len() {
        resolve_world_matrix(
            index,
            local_matrices,
            parent_map,
            &mut worlds,
            &mut resolved,
        );
    }

    worlds
}

fn resolve_world_matrix(
    index: usize,
    local_matrices: &[Matrix4<f32>],
    parent_map: &HashMap<usize, usize>,
    worlds: &mut [Matrix4<f32>],
    resolved: &mut [bool],
) {
    if resolved[index] {
        return;
    }

    let world = if let Some(parent_index) = parent_map.get(&index).copied() {
        resolve_world_matrix(parent_index, local_matrices, parent_map, worlds, resolved);
        worlds[parent_index] * local_matrices[index]
    } else {
        local_matrices[index]
    };

    worlds[index] = world;
    resolved[index] = true;
}

/// Write a MAT4 (column-major) to binary as little-endian f32 sequence.
fn write_mat4_f32_le(bin: &mut [u8], offset: usize, matrix: &Matrix4<f32>) {
    let mut cursor = offset;
    for value in matrix.as_slice() {
        let bytes = value.to_le_bytes();
        let end = cursor + 4;
        if end > bin.len() {
            return;
        }
        bin[cursor..end].copy_from_slice(&bytes);
        cursor = end;
    }
}

#[derive(Debug, Clone, Copy)]
struct AccessorMeta {
    base_offset: usize,
    stride: usize,
    count: usize,
    component_type: u64,
    accessor_type: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct PrimitiveSkinBinding {
    joints_accessor: usize,
    weights_accessor: usize,
}

/// Redistribute skin weights by removing unused joint slots and remapping
/// JOINTS_0 indices, then compact skin.joints / inverseBindMatrices.
fn optimize_skinning_weights_and_joints(json: &mut Value, bin: &mut [u8]) -> Result<()> {
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

fn accessor_meta(json: &Value, accessor_index: usize) -> Option<AccessorMeta> {
    let accessors = json.get("accessors")?.as_array()?;
    let accessor = accessors.get(accessor_index)?;
    let buffer_view_index = accessor.get("bufferView")?.as_u64()? as usize;
    let buffer_views = json.get("bufferViews")?.as_array()?;
    let buffer_view = buffer_views.get(buffer_view_index)?;

    let accessor_type = accessor.get("type")?.as_str()?;
    let element_count = match accessor_type {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        "MAT4" => 16,
        _ => return None,
    };

    let component_type = accessor.get("componentType")?.as_u64()?;
    let component_size = match component_type {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return None,
    };

    let view_offset = buffer_view
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let default_stride = element_count * component_size;
    let stride = buffer_view
        .get("byteStride")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(default_stride);

    Some(AccessorMeta {
        base_offset: view_offset + accessor_offset,
        stride,
        count: accessor.get("count")?.as_u64()? as usize,
        component_type,
        accessor_type: match accessor_type {
            "SCALAR" => "SCALAR",
            "VEC2" => "VEC2",
            "VEC3" => "VEC3",
            "VEC4" => "VEC4",
            "MAT4" => "MAT4",
            _ => return None,
        },
    })
}

fn read_joint_slot(bin: &[u8], meta: &AccessorMeta, vertex: usize, lane: usize) -> Option<u16> {
    let offset = meta.base_offset
        + vertex * meta.stride
        + lane
            * match meta.component_type {
                5121 => 1,
                5123 => 2,
                _ => return None,
            };

    match meta.component_type {
        5121 => bin.get(offset).copied().map(|value| value as u16),
        5123 => {
            let bytes = bin.get(offset..offset + 2)?;
            Some(u16::from_le_bytes([bytes[0], bytes[1]]))
        }
        _ => None,
    }
}

fn write_joint_slot(bin: &mut [u8], meta: &AccessorMeta, vertex: usize, lane: usize, value: u16) {
    let offset = meta.base_offset
        + vertex * meta.stride
        + lane
            * match meta.component_type {
                5121 => 1,
                5123 => 2,
                _ => return,
            };

    match meta.component_type {
        5121 => {
            if let Some(byte) = bin.get_mut(offset) {
                *byte = value.min(u8::MAX as u16) as u8;
            }
        }
        5123 => {
            if let Some(slice) = bin.get_mut(offset..offset + 2) {
                slice.copy_from_slice(&value.to_le_bytes());
            }
        }
        _ => {}
    }
}

fn read_weight_f32(bin: &[u8], meta: &AccessorMeta, vertex: usize, lane: usize) -> Option<f32> {
    if meta.component_type != 5126 {
        return None;
    }
    let offset = meta.base_offset + vertex * meta.stride + lane * 4;
    let bytes = bin.get(offset..offset + 4)?;
    Some(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn write_weight_f32(bin: &mut [u8], meta: &AccessorMeta, vertex: usize, lane: usize, value: f32) {
    if meta.component_type != 5126 {
        return;
    }
    let offset = meta.base_offset + vertex * meta.stride + lane * 4;
    if let Some(slice) = bin.get_mut(offset..offset + 4) {
        slice.copy_from_slice(&value.to_le_bytes());
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

/// Apply uniform scale to the first scene's root nodes by setting the scale
/// property. glTF skinning propagates root scale to both mesh vertices and
/// joint world transforms uniformly, so this is the correct place to apply it.
/// Apply uniform scale by setting a scale property on scene root nodes.
/// Replaced by `bake_scale_into_geometry` for better SL compatibility.
#[allow(dead_code)]
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

    // Collect skinned mesh node indices (nodes that reference a skin).
    // Per glTF spec, the node transform of a skinned mesh is supposed to be
    // ignored by renderers. However the SL viewer does apply the mesh-node
    // transform to already-skinned vertices, effectively double-scaling them.
    // We therefore skip skinned nodes and only apply scale to the skeleton root.
    let skinned_node_indices: HashSet<usize> = json
        .get("nodes")
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .enumerate()
                .filter_map(|(i, n)| n.get("skin").map(|_| i))
                .collect()
        })
        .unwrap_or_default();

    if let Some(nodes) = json.get_mut("nodes").and_then(Value::as_array_mut) {
        for node_index in root_node_indices
            .into_iter()
            .filter_map(|index| index.as_u64().map(|n| n as usize))
        {
            // Skip skinned mesh nodes to prevent double-scaling.
            if skinned_node_indices.contains(&node_index) {
                continue;
            }

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

/// Bake a uniform scale factor into geometry rather than storing it as a node
/// scale property.  This is the most universally compatible form for SL uploads
/// because some SL viewer versions apply the mesh-node transform to skinned
/// vertices (contrary to the glTF spec), which causes double-scaling when the
/// root is also scaled.
///
/// Two kinds of data are scaled:
/// 1. **Node translations**: every node's local translation is multiplied by
///    `scale_factor`.  Node scale properties are NOT modified (they would
///    compound multiplicatively with the translation baking and cause drift).
/// 2. **Mesh vertex positions**: the binary data for every unique POSITION
///    accessor is read and each XYZ f32 triple is multiplied by `scale_factor`.
///    NORMAL / TANGENT vectors are directions and must not be scaled.
///
/// After calling this function the scene no longer needs any root-level scale;
/// call `regenerate_inverse_bind_matrices` afterwards so IBMs match.
fn bake_scale_into_geometry(json: &mut Value, bin: &mut [u8], scale_factor: f32) -> Result<()> {
    if !scale_factor.is_finite() || (scale_factor - 1.0).abs() <= 1e-6 {
        return Ok(());
    }

    // --- 1. Scale all node translations ---
    let node_count = json["nodes"].as_array().map(|a| a.len()).unwrap_or(0);
    for i in 0..node_count {
        // Handle TRS form.
        if let Some(t) = json["nodes"][i]
            .get_mut("translation")
            .and_then(Value::as_array_mut)
        {
            for component in t.iter_mut() {
                if let Some(v) = component.as_f64() {
                    *component = Value::from(v * scale_factor as f64);
                }
            }
        }
        // Handle full matrix form (column-major, last column = translation).
        if let Some(m) = json["nodes"][i]
            .get_mut("matrix")
            .and_then(Value::as_array_mut)
        {
            if m.len() == 16 {
                for col_idx in [12usize, 13, 14] {
                    if let Some(v) = m[col_idx].as_f64() {
                        m[col_idx] = Value::from(v * scale_factor as f64);
                    }
                }
            }
        }
    }

    // --- 2. Scale mesh vertex POSITION data in the binary buffer ---
    // Collect unique POSITION accessor indices first to avoid double-scaling.
    let mut pos_accessor_indices: HashSet<usize> = HashSet::new();
    if let Some(meshes) = json["meshes"].as_array() {
        for mesh in meshes {
            if let Some(primitives) = mesh["primitives"].as_array() {
                for primitive in primitives {
                    if let Some(idx) = primitive
                        .pointer("/attributes/POSITION")
                        .and_then(Value::as_u64)
                        .map(|v| v as usize)
                    {
                        pos_accessor_indices.insert(idx);
                    }
                    // Also scale morph target positions if present.
                    if let Some(targets) = primitive["targets"].as_array() {
                        for target in targets {
                            if let Some(idx) = target
                                .get("POSITION")
                                .and_then(Value::as_u64)
                                .map(|v| v as usize)
                            {
                                pos_accessor_indices.insert(idx);
                            }
                        }
                    }
                }
            }
        }
    }

    let accessors = json["accessors"].as_array().cloned().unwrap_or_default();
    let buffer_views = json["bufferViews"].as_array().cloned().unwrap_or_default();

    for acc_idx in pos_accessor_indices {
        let Some(accessor) = accessors.get(acc_idx) else {
            continue;
        };
        if accessor["componentType"].as_u64().unwrap_or(0) != 5126 {
            continue; // Only handle FLOAT
        }
        if accessor["type"].as_str().unwrap_or("") != "VEC3" {
            continue;
        }
        let count = accessor["count"].as_u64().unwrap_or(0) as usize;
        let bv_idx = match accessor["bufferView"].as_u64().map(|v| v as usize) {
            Some(i) => i,
            None => continue,
        };
        let Some(bv) = buffer_views.get(bv_idx) else {
            continue;
        };
        let view_offset = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
        let acc_offset = accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
        let stride = bv["byteStride"].as_u64().map(|v| v as usize).unwrap_or(12); // VEC3 float = 12 bytes
        let base = view_offset + acc_offset;

        for i in 0..count {
            let offset = base + i * stride;
            if offset + 12 > bin.len() {
                break;
            }
            for component in 0..3usize {
                let byte_pos = offset + component * 4;
                let v = f32::from_le_bytes(bin[byte_pos..byte_pos + 4].try_into().unwrap());
                let scaled = v * scale_factor;
                bin[byte_pos..byte_pos + 4].copy_from_slice(&scaled.to_le_bytes());
            }
        }
    }

    Ok(())
}

/// Apply uniform scale by baking it into local translations/matrices for all
/// nodes in the first scene hierarchy.
#[allow(dead_code)]
fn apply_uniform_scale_to_scene_translations(json: &mut Value, scale_factor: f32) {
    if !scale_factor.is_finite() || (scale_factor - 1.0).abs() <= 1e-6 {
        return;
    }

    let root_node_indices = json
        .get("scenes")
        .and_then(Value::as_array)
        .and_then(|scenes| scenes.first())
        .and_then(|scene| scene.get("nodes"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if root_node_indices.is_empty() {
        return;
    }

    let child_map: Vec<Vec<usize>> = json
        .get("nodes")
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .map(|node| {
                    node.get("children")
                        .and_then(Value::as_array)
                        .map(|children| {
                            children
                                .iter()
                                .filter_map(|value| value.as_u64().map(|index| index as usize))
                                .collect::<Vec<usize>>()
                        })
                        .unwrap_or_default()
                })
                .collect::<Vec<Vec<usize>>>()
        })
        .unwrap_or_default();

    let mut stack: Vec<usize> = root_node_indices
        .into_iter()
        .filter_map(|value| value.as_u64().map(|index| index as usize))
        .collect();
    let mut visited = HashSet::<usize>::new();

    if let Some(nodes) = json.get_mut("nodes").and_then(Value::as_array_mut) {
        while let Some(node_index) = stack.pop() {
            if !visited.insert(node_index) {
                continue;
            }

            if let Some(node) = nodes.get_mut(node_index) {
                scale_node_translation(node, scale_factor);
            }

            if let Some(children) = child_map.get(node_index) {
                for &child in children {
                    stack.push(child);
                }
            }
        }
    }
}

fn scale_node_translation(node: &mut Value, scale_factor: f32) {
    if let Some(matrix) = node.get_mut("matrix").and_then(Value::as_array_mut)
        && matrix.len() == 16
    {
        for index in [12usize, 13, 14] {
            let current = matrix[index].as_f64().unwrap_or(0.0) as f32;
            matrix[index] = Value::from(current * scale_factor);
        }
        return;
    }

    if let Some(translation) = node.get_mut("translation").and_then(Value::as_array_mut)
        && translation.len() == 3
    {
        for value in translation {
            let current = value.as_f64().unwrap_or(0.0) as f32;
            *value = Value::from(current * scale_factor);
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

    #[test]
    fn given_invalid_humanoid_node_index_when_validating_preconditions_then_error_is_reported() {
        let input_json = serde_json::json!({
            "nodes": [
                {"name": "Node0"}
            ]
        });

        let humanoid_bone_nodes = [("hips".to_string(), 99usize)]
            .into_iter()
            .collect::<HashMap<String, usize>>();

        let issues = validate_bone_conversion_preconditions(&input_json, &humanoid_bone_nodes);
        assert!(
            issues
                .iter()
                .any(|issue| issue.code == "INVALID_BONE_NODE_INDEX")
        );
    }

    #[test]
    fn given_missing_target_bone_after_rename_when_ensuring_targets_then_error_is_returned() {
        let input_json = serde_json::json!({
            "nodes": [
                {"name": "hips"},
                {"name": "spine"}
            ]
        });

        let humanoid_bone_nodes = [("hips".to_string(), 0usize), ("spine".to_string(), 1usize)]
            .into_iter()
            .collect::<HashMap<String, usize>>();

        let result = ensure_target_bones_exist_after_rename(&input_json, &humanoid_bone_nodes);
        assert!(result.is_err());
    }

    #[test]
    fn given_upper_limb_target_node_when_applying_t_pose_then_rotation_is_identity() {
        let mut input_json = serde_json::json!({
            "nodes": [
                {
                    "name": "mShoulderLeft",
                    "rotation": [0.0, 0.0, -0.2, 0.98],
                    "translation": [1.0, 2.0, 3.0],
                    "scale": [1.0, 1.0, 1.0]
                }
            ]
        });

        let humanoid = HashMap::from([("leftUpperArm".to_string(), 0usize)]);
        normalize_sl_bone_rotations(&mut input_json, &humanoid);
        let rotation = input_json
            .pointer("/nodes/0/rotation")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        assert_eq!(
            rotation,
            vec![
                Value::from(0.0),
                Value::from(0.0),
                Value::from(0.0),
                Value::from(1.0)
            ]
        );
    }

    #[test]
    fn given_spine_with_world_xz_offsets_when_normalizing_then_world_positions_are_preserved() {
        let mut json = serde_json::json!({
            "nodes": [
                {
                    "name": "mPelvis",
                    "translation": [1.0, 0.0, 2.0],
                    "rotation": [0.0, 0.0, 0.0, 1.0],
                    "children": [1]
                },
                {
                    "name": "mTorso",
                    "translation": [0.4, 1.0, 0.3],
                    "rotation": [0.0, 0.0, 0.2, 0.98],
                    "children": [2]
                },
                {
                    "name": "mHead",
                    "translation": [0.2, 0.9, 0.1],
                    "rotation": [0.0, 0.1, 0.0, 0.99]
                }
            ]
        });

        let humanoid = HashMap::from([
            ("hips".to_string(), 0usize),
            ("spine".to_string(), 1usize),
            ("head".to_string(), 2usize),
        ]);

        let before_locals = json["nodes"]
            .as_array()
            .expect("nodes should be array")
            .iter()
            .map(node_to_local_matrix)
            .collect::<Vec<_>>();
        let before_world =
            compute_node_world_matrices(&before_locals, &collect_parent_index_map_from_json(&json));

        normalize_sl_bone_rotations(&mut json, &humanoid);

        let after_locals = json["nodes"]
            .as_array()
            .expect("nodes should be array")
            .iter()
            .map(node_to_local_matrix)
            .collect::<Vec<_>>();
        let after_world =
            compute_node_world_matrices(&after_locals, &collect_parent_index_map_from_json(&json));

        for node_index in [0usize, 1usize, 2usize] {
            let bt = Vector3::new(
                before_world[node_index][(0, 3)],
                before_world[node_index][(1, 3)],
                before_world[node_index][(2, 3)],
            );
            let at = Vector3::new(
                after_world[node_index][(0, 3)],
                after_world[node_index][(1, 3)],
                after_world[node_index][(2, 3)],
            );
            assert!((bt - at).norm() < 1e-4);

            let rotation = json["nodes"][node_index]
                .get("rotation")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            assert_eq!(
                rotation,
                vec![
                    Value::from(0.0),
                    Value::from(0.0),
                    Value::from(0.0),
                    Value::from(1.0)
                ]
            );
        }
    }

    #[test]
    fn given_messy_hierarchy_when_reconstructing_then_core_links_follow_sl_shape() {
        let mut json = serde_json::json!({
            "nodes": [
                {"name":"mPelvis", "children":[2,7]},
                {"name":"mTorso", "children":[]},
                {"name":"mChest", "children":[4]},
                {"name":"mNeck", "children":[]},
                {"name":"mHead", "children":[]},
                {"name":"mShoulderLeft", "children":[]},
                {"name":"mElbowLeft", "children":[]},
                {"name":"mWristLeft", "children":[]},
                {"name":"mHipLeft", "children":[]},
                {"name":"mKneeLeft", "children":[]},
                {"name":"mAnkleLeft", "children":[]}
            ],
            "scenes": [
                {"nodes":[0,1,2,3,4,5,6,7,8,9,10]}
            ]
        });

        let humanoid = HashMap::from([
            ("hips".to_string(), 0usize),
            ("spine".to_string(), 1usize),
            ("chest".to_string(), 2usize),
            ("neck".to_string(), 3usize),
            ("head".to_string(), 4usize),
            ("leftUpperArm".to_string(), 5usize),
            ("leftLowerArm".to_string(), 6usize),
            ("leftHand".to_string(), 7usize),
            ("leftUpperLeg".to_string(), 8usize),
            ("leftLowerLeg".to_string(), 9usize),
            ("leftFoot".to_string(), 10usize),
        ]);

        reconstruct_sl_core_hierarchy(&mut json, &humanoid);

        let hips_children = json
            .pointer("/nodes/0/children")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(hips_children.contains(&Value::from(1u64)));
        assert!(hips_children.contains(&Value::from(8u64)));

        let chest_children = json
            .pointer("/nodes/2/children")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(chest_children.contains(&Value::from(3u64)));
        assert!(chest_children.contains(&Value::from(5u64)));

        let scene_roots = json
            .pointer("/scenes/0/nodes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(scene_roots.contains(&Value::from(0u64)));
        assert!(!scene_roots.contains(&Value::from(1u64)));
        assert!(!scene_roots.contains(&Value::from(2u64)));
    }

    #[test]
    fn given_bento_humanoid_mapping_when_renaming_then_eye_jaw_and_finger_names_are_mapped() {
        let mut json = serde_json::json!({
            "nodes": [
                {"name":"leftEye"},
                {"name":"jaw"},
                {"name":"leftIndexProximal"},
                {"name":"leftIndexIntermediate"},
                {"name":"leftIndexDistal"}
            ]
        });

        let humanoid = HashMap::from([
            ("leftEye".to_string(), 0usize),
            ("jaw".to_string(), 1usize),
            ("leftIndexProximal".to_string(), 2usize),
            ("leftIndexIntermediate".to_string(), 3usize),
            ("leftIndexDistal".to_string(), 4usize),
        ]);

        rename_bones(&mut json, &humanoid);

        assert_eq!(
            json.pointer("/nodes/0/name").and_then(Value::as_str),
            Some("mEyeLeft")
        );
        assert_eq!(
            json.pointer("/nodes/1/name").and_then(Value::as_str),
            Some("mFaceJaw")
        );
        assert_eq!(
            json.pointer("/nodes/2/name").and_then(Value::as_str),
            Some("mHandIndex1Left")
        );
        assert_eq!(
            json.pointer("/nodes/3/name").and_then(Value::as_str),
            Some("mHandIndex2Left")
        );
        assert_eq!(
            json.pointer("/nodes/4/name").and_then(Value::as_str),
            Some("mHandIndex3Left")
        );
    }

    #[test]
    fn given_bento_hand_chain_when_reconstructing_then_finger_chain_is_relinked() {
        let mut json = serde_json::json!({
            "nodes": [
                {"name":"mWristLeft", "children":[]},
                {"name":"mHandIndex1Left", "children":[]},
                {"name":"mHandIndex2Left", "children":[]},
                {"name":"mHandIndex3Left", "children":[]}
            ],
            "scenes": [
                {"nodes":[0,1,2,3]}
            ]
        });

        let humanoid = HashMap::from([
            ("leftHand".to_string(), 0usize),
            ("leftIndexProximal".to_string(), 1usize),
            ("leftIndexIntermediate".to_string(), 2usize),
            ("leftIndexDistal".to_string(), 3usize),
        ]);

        reconstruct_sl_core_hierarchy(&mut json, &humanoid);

        let hand_children = json
            .pointer("/nodes/0/children")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(hand_children.contains(&Value::from(1u64)));

        let idx1_children = json
            .pointer("/nodes/1/children")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(idx1_children.contains(&Value::from(2u64)));

        let idx2_children = json
            .pointer("/nodes/2/children")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(idx2_children.contains(&Value::from(3u64)));
    }

    #[test]
    fn given_unused_joint_slot_when_optimizing_skinning_then_joints_and_ibm_are_compacted() {
        let mut json = serde_json::json!({
            "nodes": [
                { "mesh": 0, "skin": 0 },
                { "name": "jointA" },
                { "name": "jointB" },
                { "name": "jointUnused" }
            ],
            "meshes": [
                {
                    "primitives": [
                        {
                            "attributes": {
                                "JOINTS_0": 0,
                                "WEIGHTS_0": 1
                            }
                        }
                    ]
                }
            ],
            "skins": [
                {
                    "joints": [1, 2, 3],
                    "inverseBindMatrices": 2
                }
            ],
            "accessors": [
                { "bufferView": 0, "componentType": 5121, "count": 2, "type": "VEC4" },
                { "bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC4" },
                { "bufferView": 2, "componentType": 5126, "count": 3, "type": "MAT4" }
            ],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 0, "byteLength": 8 },
                { "buffer": 0, "byteOffset": 8, "byteLength": 32 },
                { "buffer": 0, "byteOffset": 40, "byteLength": 192 }
            ],
            "buffers": [
                { "byteLength": 232 }
            ]
        });

        let mut bin = vec![0u8; 232];

        bin[0..8].copy_from_slice(&[0, 1, 2, 2, 1, 0, 2, 2]);

        let weights = [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
        for (index, value) in weights.iter().enumerate() {
            let offset = 8 + index * 4;
            bin[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }

        optimize_skinning_weights_and_joints(&mut json, &mut bin)
            .expect("optimization should succeed");

        let joints_len = json
            .pointer("/skins/0/joints")
            .and_then(Value::as_array)
            .map(|array| array.len())
            .unwrap_or(0);
        assert_eq!(joints_len, 2);

        let ibm_count = json
            .pointer("/accessors/2/count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        assert_eq!(ibm_count, 2);
    }
}
