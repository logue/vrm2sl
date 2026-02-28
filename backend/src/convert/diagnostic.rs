use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use gltf::{binary::Glb, import};
use nalgebra::{Matrix4, Vector3};
use serde::Serialize;
use serde_json::Value;

use super::gltf_utils::{
    accessor_meta, collect_parent_index_map_from_json, compute_node_world_matrices,
    node_to_local_matrix, read_mat4_from_accessor,
};
use super::types::TextureInfo;

// ─── Diagnostic structs ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub(super) struct MeshSkinLinkDiagnostic {
    node_index: usize,
    node_name: Option<String>,
    skin_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct JointDiagnostic {
    slot: usize,
    node_index: usize,
    node_name: Option<String>,
    parent_index: Option<usize>,
    parent_name: Option<String>,
    local_translation: [f32; 3],
    local_rotation: [f32; 4],
    world_translation: [f32; 3],
    ibm_translation: Option<[f32; 3]>,
    bind_world_translation: Option<[f32; 3]>,
    world_bind_distance: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SkinDiagnostic {
    skin_index: usize,
    skeleton_index: Option<usize>,
    skeleton_name: Option<String>,
    joints_count: usize,
    inverse_bind_accessor: Option<usize>,
    joints: Vec<JointDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ConversionDiagnosticLog {
    output_path: String,
    scale_factor: f32,
    node_count: usize,
    skin_count: usize,
    mesh_nodes_with_skin: Vec<MeshSkinLinkDiagnostic>,
    skins: Vec<SkinDiagnostic>,
}

// ─── Path helper ──────────────────────────────────────────────────────────────

pub(super) fn diagnostic_log_path_for_output(output_path: &Path) -> PathBuf {
    output_path.with_extension("diagnostic.json")
}

// ─── Diagnostic writer ────────────────────────────────────────────────────────

pub(super) fn write_conversion_diagnostic_log(
    output_path: &Path,
    diagnostic_path: &Path,
    scale_factor: f32,
) -> Result<()> {
    let bytes = fs::read(output_path)
        .with_context(|| format!("failed to read output file: {}", output_path.display()))?;
    let glb = Glb::from_slice(&bytes).context("output file is not a GLB container")?;
    let json: Value = serde_json::from_slice(glb.json.as_ref())
        .context("failed to parse glTF JSON from output GLB")?;
    let bin = glb.bin.map(|chunk| chunk.into_owned()).unwrap_or_default();

    let nodes = json
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let parent_map = collect_parent_index_map_from_json(&json);
    let locals: Vec<Matrix4<f32>> = nodes.iter().map(node_to_local_matrix).collect();
    let worlds = compute_node_world_matrices(&locals, &parent_map);

    let mesh_nodes_with_skin = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.get("mesh").is_some())
        .map(|(node_index, node)| MeshSkinLinkDiagnostic {
            node_index,
            node_name: node
                .get("name")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            skin_index: node.get("skin").and_then(Value::as_u64).map(|v| v as usize),
        })
        .collect::<Vec<_>>();

    let mut skins_out = Vec::<SkinDiagnostic>::new();
    if let Some(skins) = json.get("skins").and_then(Value::as_array) {
        for (skin_index, skin) in skins.iter().enumerate() {
            let skeleton_index = skin
                .get("skeleton")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            let skeleton_name = skeleton_index
                .and_then(|index| nodes.get(index))
                .and_then(|node| node.get("name"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);

            let inverse_bind_accessor = skin
                .get("inverseBindMatrices")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            let inverse_bind_meta = inverse_bind_accessor
                .and_then(|accessor_index| accessor_meta(&json, accessor_index))
                .filter(|meta| meta.component_type == 5126 && meta.accessor_type == "MAT4");

            let joints = skin
                .get("joints")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();

            let mut joint_out = Vec::<JointDiagnostic>::new();
            for (slot, joint_value) in joints.iter().enumerate() {
                let Some(node_index) = joint_value.as_u64().map(|v| v as usize) else {
                    continue;
                };

                let node = nodes.get(node_index);
                let node_name = node
                    .and_then(|n| n.get("name"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);

                let parent_index = parent_map.get(&node_index).copied();
                let parent_name = parent_index
                    .and_then(|index| nodes.get(index))
                    .and_then(|n| n.get("name"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);

                let local = locals
                    .get(node_index)
                    .copied()
                    .unwrap_or_else(Matrix4::<f32>::identity);
                let world = worlds
                    .get(node_index)
                    .copied()
                    .unwrap_or_else(Matrix4::<f32>::identity);

                let local_translation = [local[(0, 3)], local[(1, 3)], local[(2, 3)]];
                let world_translation = [world[(0, 3)], world[(1, 3)], world[(2, 3)]];
                let local_rotation = node
                    .and_then(|n| n.get("rotation"))
                    .and_then(Value::as_array)
                    .filter(|r| r.len() == 4)
                    .map(|r| {
                        [
                            r[0].as_f64().unwrap_or(0.0) as f32,
                            r[1].as_f64().unwrap_or(0.0) as f32,
                            r[2].as_f64().unwrap_or(0.0) as f32,
                            r[3].as_f64().unwrap_or(1.0) as f32,
                        ]
                    })
                    .unwrap_or([0.0, 0.0, 0.0, 1.0]);

                let ibm_matrix = inverse_bind_meta
                    .as_ref()
                    .and_then(|meta| read_mat4_from_accessor(&bin, meta, slot));
                let ibm_translation = ibm_matrix
                    .as_ref()
                    .map(|matrix| [matrix[(0, 3)], matrix[(1, 3)], matrix[(2, 3)]]);
                let bind_world_translation = ibm_matrix
                    .as_ref()
                    .and_then(|matrix| matrix.try_inverse())
                    .map(|matrix| [matrix[(0, 3)], matrix[(1, 3)], matrix[(2, 3)]]);
                let world_bind_distance = bind_world_translation.map(|bind| {
                    let world_v = Vector3::new(
                        world_translation[0],
                        world_translation[1],
                        world_translation[2],
                    );
                    let bind_v = Vector3::new(bind[0], bind[1], bind[2]);
                    (world_v - bind_v).norm()
                });

                joint_out.push(JointDiagnostic {
                    slot,
                    node_index,
                    node_name,
                    parent_index,
                    parent_name,
                    local_translation,
                    local_rotation,
                    world_translation,
                    ibm_translation,
                    bind_world_translation,
                    world_bind_distance,
                });
            }

            skins_out.push(SkinDiagnostic {
                skin_index,
                skeleton_index,
                skeleton_name,
                joints_count: joints.len(),
                inverse_bind_accessor,
                joints: joint_out,
            });
        }
    }

    let diagnostic = ConversionDiagnosticLog {
        output_path: output_path.display().to_string(),
        scale_factor,
        node_count: nodes.len(),
        skin_count: skins_out.len(),
        mesh_nodes_with_skin,
        skins: skins_out,
    };

    let json_bytes = serde_json::to_vec_pretty(&diagnostic)
        .context("failed to serialize conversion diagnostic JSON")?;
    fs::write(diagnostic_path, json_bytes).with_context(|| {
        format!(
            "failed to write conversion diagnostic log: {}",
            diagnostic_path.display()
        )
    })?;

    Ok(())
}

// ─── Post-export helpers ──────────────────────────────────────────────────────

/// Collect texture dimensions from an exported GLB output file.
pub(super) fn collect_output_texture_infos(output_path: &Path) -> Result<Vec<TextureInfo>> {
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
pub(super) fn parse_glb_json(input_path: &Path) -> Result<Value> {
    let input_bytes = fs::read(input_path)
        .with_context(|| format!("failed to read input file: {}", input_path.display()))?;
    let input_glb = Glb::from_slice(&input_bytes).context("input VRM is not a GLB container")?;
    serde_json::from_slice(input_glb.json.as_ref())
        .context("failed to parse glTF JSON chunk from VRM")
}
