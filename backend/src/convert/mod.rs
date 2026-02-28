mod diagnostic;
mod geometry;
mod gltf_utils;
mod skeleton;
mod skinning;
mod types;
mod validation;

use std::{borrow::Cow, collections::HashMap, fs, io::Cursor, path::Path};

use anyhow::{Context, Result, bail};
use gltf::{binary::Glb, import};
use image::ImageFormat;
use serde_json::Value;

use crate::texture::{ResizeInterpolation, resize_texture_to_max};

// Re-export public types for callers of this module.
pub use types::{
    AnalysisReport, ConversionReport, ConvertOptions, Severity, TextureInfo, UploadFeeEstimate,
    ValidationIssue,
};

// Pull in sub-module helpers used in the orchestration functions below.
use diagnostic::{
    collect_output_texture_infos, diagnostic_log_path_for_output, parse_glb_json,
    write_conversion_diagnostic_log,
};
use geometry::{bake_scale_into_geometry, collect_mesh_statistics, estimate_height_cm};
use skeleton::{
    ensure_target_bones_exist_after_rename, normalize_sl_bone_rotations,
    promote_pelvis_to_scene_root, reconstruct_sl_core_hierarchy, regenerate_inverse_bind_matrices,
    rename_bones, set_skin_skeleton_root, validate_bone_conversion_preconditions,
};
use skinning::{optimize_skinning_weights_and_joints, remap_unmapped_bone_weights};
use validation::{
    collect_mapped_bones, collect_missing_required_bones, collect_node_names,
    collect_parent_index_map, estimate_texture_fee, extract_author, extract_humanoid_bone_nodes,
    extract_model_name, remove_unsupported_features, remove_vrm_extensions_and_extras,
    validate_hierarchy, validate_vroid_model,
};

// ─── Public API ───────────────────────────────────────────────────────────────

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

    let diagnostic_path = diagnostic_log_path_for_output(output_path);
    write_conversion_diagnostic_log(output_path, &diagnostic_path, computed_scale_factor)?;

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

    let mut issues = analysis.issues;
    issues.push(ValidationIssue {
        severity: Severity::Info,
        code: "DIAGNOSTIC_LOG_WRITTEN".to_string(),
        message: format!(
            "[INFO] Conversion diagnostic log written: {}",
            diagnostic_path.display()
        ),
    });

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
        issues,
    })
}

// ─── Private orchestration ────────────────────────────────────────────────────

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
    // Clean up wrapper nodes above mPelvis.  Keeps the topmost non-SL
    // ancestor as an identity-transform root so that skin.skeleton can
    // reference a node with no positional offset, preventing the SL viewer
    // from injecting an unwanted transform into the skinning pipeline.
    let identity_root = promote_pelvis_to_scene_root(&mut json, humanoid_bone_nodes);
    // Set skin.skeleton to the identity root node (or mPelvis if none
    // existed) so every importer agrees on the skeleton root.
    set_skin_skeleton_root(&mut json, humanoid_bone_nodes, identity_root);
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

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use nalgebra::Vector3;
    use serde_json::Value;

    use super::gltf_utils::{
        collect_parent_index_map_from_json, compute_node_world_matrices, node_to_local_matrix,
    };
    use super::skeleton::{
        ensure_target_bones_exist_after_rename, normalize_sl_bone_rotations,
        promote_pelvis_to_scene_root, reconstruct_sl_core_hierarchy, rename_bones,
        validate_bone_conversion_preconditions,
    };
    use super::skinning::optimize_skinning_weights_and_joints;
    use super::validation::{
        estimate_texture_fee, extract_humanoid_bone_nodes, projected_texture_size,
        validate_hierarchy,
    };
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

    #[test]
    fn given_root_wrapper_when_promoting_pelvis_then_identity_root_is_kept() {
        let mut json = serde_json::json!({
            "nodes": [
                {
                    "name": "Root",
                    "translation": [0.5, 0.0, 0.1],
                    "children": [1, 2]
                },
                {
                    "name": "mPelvis",
                    "translation": [0.0, 1.0, 0.0],
                    "rotation": [0.0, 0.0, 0.0, 1.0],
                    "children": []
                },
                {
                    "name": "Body",
                    "translation": [0.0, 0.0, 0.0],
                    "skin": 0
                }
            ],
            "scenes": [
                {"nodes": [0]}
            ]
        });

        let humanoid = HashMap::from([("hips".to_string(), 1usize)]);
        let identity_root = promote_pelvis_to_scene_root(&mut json, &humanoid);

        // The identity root should be the original Root node (index 0).
        assert_eq!(identity_root, Some(0));

        // Root should still be in the scene as the identity skeleton root.
        let scene_nodes = json
            .pointer("/scenes/0/nodes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(
            scene_nodes.iter().any(|v| v.as_u64() == Some(0)),
            "Root (identity) should remain in scene: {scene_nodes:?}"
        );

        // mPelvis should NOT be a direct scene child (it's under Root now).
        assert!(
            !scene_nodes.iter().any(|v| v.as_u64() == Some(1)),
            "mPelvis should not be a direct scene node: {scene_nodes:?}"
        );

        // Body mesh should be promoted to scene root.
        assert!(
            scene_nodes.iter().any(|v| v.as_u64() == Some(2)),
            "Body should be promoted to scene: {scene_nodes:?}"
        );

        // Root's transform should be identity.
        let root_t = json
            .pointer("/nodes/0/translation")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let root_tx = root_t.first().and_then(Value::as_f64).unwrap_or(99.0);
        assert!((root_tx).abs() < 1e-4, "Root translation should be zero");

        // Root's children should contain only mPelvis.
        let root_children = json
            .pointer("/nodes/0/children")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(root_children, vec![Value::from(1u64)]);

        // mPelvis translation should equal its world position.
        // Original: Root t=[0.5,0,0.1] + Pelvis t=[0,1,0] → world [0.5,1,0.1]
        let t = json
            .pointer("/nodes/1/translation")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let x = t.first().and_then(Value::as_f64).unwrap_or(0.0);
        let y = t.get(1).and_then(Value::as_f64).unwrap_or(0.0);
        assert!(
            (x - 0.5).abs() < 1e-4,
            "mPelvis X should be ~0.5 (world) but got {x}"
        );
        assert!(
            (y - 1.0).abs() < 1e-4,
            "mPelvis Y should be ~1.0 (world) but got {y}"
        );
    }
}
