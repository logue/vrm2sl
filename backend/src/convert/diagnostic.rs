use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use gltf::{binary::Glb, import};
use nalgebra::{Matrix4, Vector3, Vector4};
use serde::Serialize;
use serde_json::Value;

use super::gltf_utils::{
    accessor_meta, collect_parent_index_map_from_json, compute_node_world_matrices,
    node_to_local_matrix, read_joint_slot, read_mat4_from_accessor, read_weight_f32,
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
pub(super) struct VertexInfluenceDiagnostic {
    joint_name: String,
    weight: f32,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct FingerRepresentativeVertexDiagnostic {
    side: String,
    finger: String,
    segment: String,
    joint_name: String,
    mesh_node_index: usize,
    mesh_node_name: Option<String>,
    primitive_index: usize,
    vertex_index: usize,
    dominant_joint_weight: f32,
    family_weight: f32,
    thumb_weight: f32,
    wrist_weight: f32,
    local_position: [f32; 3],
    world_position: [f32; 3],
    influences: Vec<VertexInfluenceDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ConversionDiagnosticLog {
    output_path: String,
    scale_factor: f32,
    node_count: usize,
    skin_count: usize,
    mesh_nodes_with_skin: Vec<MeshSkinLinkDiagnostic>,
    finger_representative_vertices: Vec<FingerRepresentativeVertexDiagnostic>,
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
    let finger_representative_vertices =
        collect_finger_representative_vertices(&json, &bin, &nodes, &mesh_nodes_with_skin, &worlds);

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
        finger_representative_vertices,
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

fn collect_finger_representative_vertices(
    json: &Value,
    bin: &[u8],
    nodes: &[Value],
    mesh_nodes_with_skin: &[MeshSkinLinkDiagnostic],
    worlds: &[Matrix4<f32>],
) -> Vec<FingerRepresentativeVertexDiagnostic> {
    let mut representatives = HashMap::<String, FingerRepresentativeVertexDiagnostic>::new();

    for mesh_link in mesh_nodes_with_skin {
        let Some(mesh_index) = nodes
            .get(mesh_link.node_index)
            .and_then(|node| node.get("mesh"))
            .and_then(Value::as_u64)
            .map(|value| value as usize)
        else {
            continue;
        };
        let Some(skin_index) = mesh_link.skin_index else {
            continue;
        };
        let Some(mesh) = json
            .get("meshes")
            .and_then(Value::as_array)
            .and_then(|meshes| meshes.get(mesh_index))
        else {
            continue;
        };
        let Some(joints) = json
            .get("skins")
            .and_then(Value::as_array)
            .and_then(|skins| skins.get(skin_index))
            .and_then(|skin| skin.get("joints"))
            .and_then(Value::as_array)
        else {
            continue;
        };

        let Some(node_world) = worlds.get(mesh_link.node_index) else {
            continue;
        };

        let Some(primitives) = mesh.get("primitives").and_then(Value::as_array) else {
            continue;
        };

        for (primitive_index, primitive) in primitives.iter().enumerate() {
            let Some(attributes) = primitive.get("attributes") else {
                continue;
            };
            let Some(position_accessor) = attributes
                .get("POSITION")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
            else {
                continue;
            };
            let Some(joints_0_accessor) = attributes
                .get("JOINTS_0")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
            else {
                continue;
            };
            let Some(weights_0_accessor) = attributes
                .get("WEIGHTS_0")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
            else {
                continue;
            };

            let Some(position_meta) = accessor_meta(json, position_accessor)
                .filter(|meta| meta.component_type == 5126 && meta.accessor_type == "VEC3")
            else {
                continue;
            };
            let Some(joints_0_meta) =
                accessor_meta(json, joints_0_accessor).filter(|meta| meta.accessor_type == "VEC4")
            else {
                continue;
            };
            let Some(weights_0_meta) = accessor_meta(json, weights_0_accessor)
                .filter(|meta| meta.component_type == 5126 && meta.accessor_type == "VEC4")
            else {
                continue;
            };
            let joints_1_meta = attributes
                .get("JOINTS_1")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
                .and_then(|index| accessor_meta(json, index))
                .filter(|meta| meta.accessor_type == "VEC4");
            let weights_1_meta = attributes
                .get("WEIGHTS_1")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
                .and_then(|index| accessor_meta(json, index))
                .filter(|meta| meta.component_type == 5126 && meta.accessor_type == "VEC4");

            let vertex_count = position_meta
                .count
                .min(joints_0_meta.count)
                .min(weights_0_meta.count);
            for vertex_index in 0..vertex_count {
                let Some(local_position) = read_vec3_f32(bin, &position_meta, vertex_index) else {
                    continue;
                };
                let Some(candidate) = build_finger_vertex_candidate(
                    bin,
                    nodes,
                    joints,
                    &joints_0_meta,
                    &weights_0_meta,
                    joints_1_meta.as_ref(),
                    weights_1_meta.as_ref(),
                    vertex_index,
                ) else {
                    continue;
                };

                let world_position = transform_point(node_world, &local_position);
                let key = format!(
                    "{}:{}:{}",
                    candidate.side, candidate.finger, candidate.segment
                );
                let next = FingerRepresentativeVertexDiagnostic {
                    side: candidate.side,
                    finger: candidate.finger,
                    segment: candidate.segment,
                    joint_name: candidate.joint_name,
                    mesh_node_index: mesh_link.node_index,
                    mesh_node_name: mesh_link.node_name.clone(),
                    primitive_index,
                    vertex_index,
                    dominant_joint_weight: candidate.dominant_joint_weight,
                    family_weight: candidate.family_weight,
                    thumb_weight: candidate.thumb_weight,
                    wrist_weight: candidate.wrist_weight,
                    local_position,
                    world_position,
                    influences: candidate.influences,
                };

                match representatives.get(&key) {
                    Some(current)
                        if current.dominant_joint_weight > next.dominant_joint_weight
                            || (current.dominant_joint_weight == next.dominant_joint_weight
                                && current.family_weight >= next.family_weight) => {}
                    _ => {
                        representatives.insert(key, next);
                    }
                }
            }
        }
    }

    let mut out = representatives.into_values().collect::<Vec<_>>();
    out.sort_by(|a, b| {
        a.side
            .cmp(&b.side)
            .then(a.finger.cmp(&b.finger))
            .then(a.segment.cmp(&b.segment))
    });
    out
}

#[derive(Debug, Clone)]
struct FingerVertexCandidate {
    side: String,
    finger: String,
    segment: String,
    joint_name: String,
    dominant_joint_weight: f32,
    family_weight: f32,
    thumb_weight: f32,
    wrist_weight: f32,
    influences: Vec<VertexInfluenceDiagnostic>,
}

fn build_finger_vertex_candidate(
    bin: &[u8],
    nodes: &[Value],
    joints: &[Value],
    joints_0_meta: &super::gltf_utils::AccessorMeta,
    weights_0_meta: &super::gltf_utils::AccessorMeta,
    joints_1_meta: Option<&super::gltf_utils::AccessorMeta>,
    weights_1_meta: Option<&super::gltf_utils::AccessorMeta>,
    vertex_index: usize,
) -> Option<FingerVertexCandidate> {
    let mut influence_by_joint = HashMap::<String, f32>::new();
    accumulate_vertex_influences(
        bin,
        nodes,
        joints,
        joints_0_meta,
        weights_0_meta,
        vertex_index,
        &mut influence_by_joint,
    );
    if let (Some(joints_1_meta), Some(weights_1_meta)) = (joints_1_meta, weights_1_meta) {
        accumulate_vertex_influences(
            bin,
            nodes,
            joints,
            joints_1_meta,
            weights_1_meta,
            vertex_index,
            &mut influence_by_joint,
        );
    }
    if influence_by_joint.is_empty() {
        return None;
    }

    let mut family_weight = 0.0f32;
    let mut thumb_weight = 0.0f32;
    let mut wrist_weight = 0.0f32;
    let mut dominant_joint_name = None::<String>;
    let mut dominant_joint_weight = 0.0f32;
    let mut side = None::<String>;
    let mut finger = None::<String>;
    let mut segment = None::<String>;

    let mut influences = influence_by_joint
        .iter()
        .map(|(joint_name, weight)| VertexInfluenceDiagnostic {
            joint_name: joint_name.clone(),
            weight: *weight,
        })
        .collect::<Vec<_>>();
    influences.sort_by(|a, b| b.weight.total_cmp(&a.weight));

    for influence in &influences {
        match classify_finger_joint(&influence.joint_name) {
            Some((joint_side, joint_finger, joint_segment)) => {
                family_weight += influence.weight;
                if influence.weight > dominant_joint_weight {
                    dominant_joint_weight = influence.weight;
                    dominant_joint_name = Some(influence.joint_name.clone());
                    side = Some(joint_side.to_string());
                    finger = Some(joint_finger.to_string());
                    segment = Some(joint_segment.to_string());
                }
            }
            None if influence.joint_name.contains("HandThumb") => {
                thumb_weight += influence.weight;
            }
            None if influence.joint_name.contains("Wrist") => {
                wrist_weight += influence.weight;
            }
            None => {}
        }
    }

    if dominant_joint_weight <= 0.0 || thumb_weight > 1e-5 || wrist_weight > 1e-5 {
        return None;
    }

    Some(FingerVertexCandidate {
        side: side?,
        finger: finger?,
        segment: segment?,
        joint_name: dominant_joint_name?,
        dominant_joint_weight,
        family_weight,
        thumb_weight,
        wrist_weight,
        influences,
    })
}

fn accumulate_vertex_influences(
    bin: &[u8],
    nodes: &[Value],
    joints: &[Value],
    joints_meta: &super::gltf_utils::AccessorMeta,
    weights_meta: &super::gltf_utils::AccessorMeta,
    vertex_index: usize,
    influence_by_joint: &mut HashMap<String, f32>,
) {
    for lane in 0..4 {
        let Some(weight) = read_weight_f32(bin, weights_meta, vertex_index, lane) else {
            continue;
        };
        if weight <= 1e-6 {
            continue;
        }
        let Some(slot) = read_joint_slot(bin, joints_meta, vertex_index, lane) else {
            continue;
        };
        let Some(joint_index) = joints
            .get(slot as usize)
            .and_then(Value::as_u64)
            .map(|value| value as usize)
        else {
            continue;
        };
        let Some(joint_name) = nodes
            .get(joint_index)
            .and_then(|node| node.get("name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
        else {
            continue;
        };
        *influence_by_joint.entry(joint_name).or_insert(0.0) += weight;
    }
}

fn classify_finger_joint(joint_name: &str) -> Option<(&'static str, &'static str, &'static str)> {
    let side = if joint_name.ends_with("Left") {
        "Left"
    } else if joint_name.ends_with("Right") {
        "Right"
    } else {
        return None;
    };

    let finger = if joint_name.contains("HandIndex") {
        "Index"
    } else if joint_name.contains("HandMiddle") {
        "Middle"
    } else if joint_name.contains("HandRing") {
        "Ring"
    } else if joint_name.contains("HandPinky") {
        "Pinky"
    } else {
        return None;
    };

    let segment = if joint_name.contains('1') {
        "Proximal"
    } else if joint_name.contains('2') {
        "Intermediate"
    } else if joint_name.contains('3') {
        "Distal"
    } else {
        return None;
    };

    Some((side, finger, segment))
}

fn read_vec3_f32(
    bin: &[u8],
    meta: &super::gltf_utils::AccessorMeta,
    index: usize,
) -> Option<[f32; 3]> {
    if meta.component_type != 5126 || meta.accessor_type != "VEC3" || index >= meta.count {
        return None;
    }

    let offset = meta.base_offset + index * meta.stride;
    let x = f32::from_le_bytes(bin.get(offset..offset + 4)?.try_into().ok()?);
    let y = f32::from_le_bytes(bin.get(offset + 4..offset + 8)?.try_into().ok()?);
    let z = f32::from_le_bytes(bin.get(offset + 8..offset + 12)?.try_into().ok()?);
    Some([x, y, z])
}

fn transform_point(matrix: &Matrix4<f32>, position: &[f32; 3]) -> [f32; 3] {
    let result = matrix * Vector4::new(position[0], position[1], position[2], 1.0);
    [result.x, result.y, result.z]
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
