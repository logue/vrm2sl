use std::collections::HashMap;

use nalgebra::{Matrix3, Matrix4, Quaternion, Translation3, UnitQuaternion, Vector3};
use serde_json::Value;

// ─── Accessor metadata ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub(super) struct AccessorMeta {
    pub(super) base_offset: usize,
    pub(super) stride: usize,
    pub(super) count: usize,
    pub(super) component_type: u64,
    pub(super) accessor_type: &'static str,
}

/// Primitive skin binding: indices of the JOINTS_0 and WEIGHTS_0 accessors.
#[derive(Debug, Clone, Copy)]
pub(super) struct PrimitiveSkinBinding {
    pub(super) joints_accessor: usize,
    pub(super) weights_accessor: usize,
}

// ─── Accessor I/O ─────────────────────────────────────────────────────────────

pub(super) fn accessor_meta(json: &Value, accessor_index: usize) -> Option<AccessorMeta> {
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

pub(super) fn read_joint_slot(
    bin: &[u8],
    meta: &AccessorMeta,
    vertex: usize,
    lane: usize,
) -> Option<u16> {
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

pub(super) fn write_joint_slot(
    bin: &mut [u8],
    meta: &AccessorMeta,
    vertex: usize,
    lane: usize,
    value: u16,
) {
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

pub(super) fn read_weight_f32(
    bin: &[u8],
    meta: &AccessorMeta,
    vertex: usize,
    lane: usize,
) -> Option<f32> {
    if meta.component_type != 5126 {
        return None;
    }
    let offset = meta.base_offset + vertex * meta.stride + lane * 4;
    let bytes = bin.get(offset..offset + 4)?;
    Some(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub(super) fn write_weight_f32(
    bin: &mut [u8],
    meta: &AccessorMeta,
    vertex: usize,
    lane: usize,
    value: f32,
) {
    if meta.component_type != 5126 {
        return;
    }
    let offset = meta.base_offset + vertex * meta.stride + lane * 4;
    if let Some(slice) = bin.get_mut(offset..offset + 4) {
        slice.copy_from_slice(&value.to_le_bytes());
    }
}

/// Write a MAT4 (column-major) to binary as little-endian f32 sequence.
pub(super) fn write_mat4_f32_le(bin: &mut [u8], offset: usize, matrix: &Matrix4<f32>) {
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

pub(super) fn read_mat4_from_accessor(
    bin: &[u8],
    meta: &AccessorMeta,
    index: usize,
) -> Option<Matrix4<f32>> {
    if meta.accessor_type != "MAT4" || meta.component_type != 5126 {
        return None;
    }
    if index >= meta.count {
        return None;
    }

    let offset = meta.base_offset + index * meta.stride;
    if offset + 64 > bin.len() {
        return None;
    }

    let mut values = [0.0f32; 16];
    for (i, value) in values.iter_mut().enumerate() {
        let byte_offset = offset + i * 4;
        let bytes = bin.get(byte_offset..byte_offset + 4)?;
        *value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }

    Some(Matrix4::from_column_slice(&values))
}

// ─── Node hierarchy helpers ───────────────────────────────────────────────────

/// Collect child→parent node index mapping from glTF JSON.
pub(super) fn collect_parent_index_map_from_json(json: &Value) -> HashMap<usize, usize> {
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

/// Collect all node names from the glTF JSON nodes array.
pub(super) fn collect_node_name_set_from_json(json: &Value) -> std::collections::HashSet<String> {
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

/// Build local transform matrix from a glTF node JSON object.
pub(super) fn node_to_local_matrix(node: &Value) -> Matrix4<f32> {
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
pub(super) fn compute_node_world_matrices(
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

// ─── Node TRS write ───────────────────────────────────────────────────────────

/// Decompose and write a local transform matrix back into a glTF node as TRS.
pub(super) fn set_node_local_matrix(node: &mut Value, matrix: &Matrix4<f32>) {
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
