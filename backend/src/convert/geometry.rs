use std::collections::HashSet;

use anyhow::Result;
use gltf::{Document, Semantic};
use nalgebra::Vector3;
use serde_json::Value;

use super::types::{Severity, ValidationIssue};

// ─── Mesh statistics ──────────────────────────────────────────────────────────

/// Collect total mesh statistics and hard-limit validation issues.
pub(super) fn collect_mesh_statistics(
    document: &Document,
) -> (usize, usize, Vec<ValidationIssue>) {
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

/// Estimate avatar height in centimeters from mesh Y extents.
pub(super) fn estimate_height_cm(
    document: &Document,
    buffers: &[gltf::buffer::Data],
) -> Option<f32> {
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader =
                primitive.reader(|buffer| buffers.get(buffer.index()).map(|b| &b.0[..]));
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

// ─── Scale baking ─────────────────────────────────────────────────────────────

/// Bake a uniform scale factor into geometry rather than storing it as a node
/// scale property.
///
/// This is the most universally compatible form for SL uploads because some SL
/// viewer versions apply the mesh-node transform to skinned vertices (contrary
/// to the glTF spec), which causes double-scaling when the root is also scaled.
///
/// Two kinds of data are scaled:
/// 1. **Node translations**: every node's local translation is multiplied.
/// 2. **Mesh vertex positions**: the binary POSITION data for every unique
///    accessor is scaled in-place.
pub(super) fn bake_scale_into_geometry(
    json: &mut Value,
    bin: &mut [u8],
    scale_factor: f32,
) -> Result<()> {
    if !scale_factor.is_finite() || (scale_factor - 1.0).abs() <= 1e-6 {
        return Ok(());
    }

    // --- 1. Scale all node translations ---
    let node_count = json["nodes"].as_array().map(|a| a.len()).unwrap_or(0);
    for i in 0..node_count {
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
            continue;
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
        let stride = bv["byteStride"].as_u64().map(|v| v as usize).unwrap_or(12);
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

// ─── Legacy / dead-code helpers ────────────────────────────────────────────────

/// Apply uniform scale to the first scene's root nodes by setting the scale
/// property.
///
/// Replaced by [`bake_scale_into_geometry`] for better SL compatibility.
#[allow(dead_code)]
pub(super) fn apply_uniform_scale_to_scene_roots(json: &mut Value, scale_factor: f32) {
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

/// Apply uniform scale by baking it into local translations/matrices for all
/// nodes in the first scene hierarchy.
#[allow(dead_code)]
pub(super) fn apply_uniform_scale_to_scene_translations(json: &mut Value, scale_factor: f32) {
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
