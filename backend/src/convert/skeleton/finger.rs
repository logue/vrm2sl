use std::collections::{HashMap, HashSet};

use anyhow::Result;
use nalgebra::{Matrix3, Matrix4, UnitQuaternion, Vector3, Vector4};
use serde_json::Value;

use super::super::gltf_utils::{
    accessor_meta, collect_parent_index_map_from_json, compute_node_world_matrices, node_to_local_matrix,
    read_joint_slot, read_weight_f32,
};
use super::super::types::{BENTO_BONE_MAP, BONE_MAP};

fn should_skip_rotation_normalization(vrm_bone_name: &str) -> bool {
    if matches!(
        vrm_bone_name,
        "leftEye" | "rightEye" | "leftHand" | "rightHand"
    ) {
        return true;
    }

    const FINGER_BASES: [&str; 10] = [
        "leftThumb",
        "leftIndex",
        "leftMiddle",
        "leftRing",
        "leftLittle",
        "rightThumb",
        "rightIndex",
        "rightMiddle",
        "rightRing",
        "rightLittle",
    ];
    const FINGER_SEGMENTS: [&str; 3] = ["Proximal", "Intermediate", "Distal"];

    FINGER_BASES.iter().any(|base| {
        vrm_bone_name.starts_with(base)
            && FINGER_SEGMENTS
                .iter()
                .any(|segment| vrm_bone_name.ends_with(segment))
    })
}

fn log_finger_transforms_for_debug(json: &Value) {
    if std::env::var("VRM2SL_DEBUG_FINGERS").is_err() {
        return;
    }

    let finger_names = [
        "mHandThumb1Left",
        "mHandThumb2Left",
        "mHandThumb3Left",
        "mHandIndex1Left",
        "mHandIndex2Left",
        "mHandIndex3Left",
        "mHandMiddle1Left",
        "mHandMiddle2Left",
        "mHandMiddle3Left",
        "mHandRing1Left",
        "mHandRing2Left",
        "mHandRing3Left",
        "mHandPinky1Left",
        "mHandPinky2Left",
        "mHandPinky3Left",
    ];

    if let Some(nodes) = json.get("nodes").and_then(Value::as_array) {
        for node in nodes.iter() {
            if let Some(name) = node.get("name").and_then(Value::as_str) {
                if finger_names.contains(&name) {
                    let rot = node.get("rotation").and_then(Value::as_array).cloned();
                    let trans = node.get("translation").and_then(Value::as_array).cloned();
                    eprintln!(
                        "[FINGER] {} rotation: {:?} translation: {:?}",
                        name, rot, trans
                    );
                }
            }
        }
    }
}

pub(in super::super) fn normalize_sl_bone_rotations(
    json: &mut Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
) -> Vec<Matrix4<f32>> {
    let mapped_sl_node_indices: HashSet<usize> = BONE_MAP
        .iter()
        .chain(BENTO_BONE_MAP.iter())
        .filter_map(|(vrm_name, _)| humanoid_bone_nodes.get(*vrm_name).copied())
        .collect();

    let rotation_normalize_node_indices: HashSet<usize> = BONE_MAP
        .iter()
        .chain(BENTO_BONE_MAP.iter())
        .filter(|(vrm_name, _)| !should_skip_rotation_normalization(vrm_name))
        .filter_map(|(vrm_name, _)| humanoid_bone_nodes.get(*vrm_name).copied())
        .collect();

    let node_locals: Vec<Matrix4<f32>> = json["nodes"]
        .as_array()
        .map(|nodes| nodes.iter().map(node_to_local_matrix).collect())
        .unwrap_or_default();
    let parent_map = collect_parent_index_map_from_json(json);
    let node_worlds_snapshot = compute_node_world_matrices(&node_locals, &parent_map);

    let node_count = json["nodes"].as_array().map(|a| a.len()).unwrap_or(0);

    let mut topo_order: Vec<usize> = Vec::with_capacity(node_count);
    {
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

    let mut effective_world = node_worlds_snapshot.clone();
    while effective_world.len() < node_count {
        effective_world.push(Matrix4::identity());
    }

    let mut original_locals = node_locals.clone();
    while original_locals.len() < node_count {
        original_locals.push(Matrix4::identity());
    }

    let extract_rotation_3x3 = |world: &Matrix4<f32>| {
        let mut basis = Matrix3::new(
            world[(0, 0)],
            world[(0, 1)],
            world[(0, 2)],
            world[(1, 0)],
            world[(1, 1)],
            world[(1, 2)],
            world[(2, 0)],
            world[(2, 1)],
            world[(2, 2)],
        );

        for c in 0..3 {
            let col_norm = (basis[(0, c)] * basis[(0, c)]
                + basis[(1, c)] * basis[(1, c)]
                + basis[(2, c)] * basis[(2, c)])
                .sqrt();
            if col_norm > 1e-8 {
                basis[(0, c)] /= col_norm;
                basis[(1, c)] /= col_norm;
                basis[(2, c)] /= col_norm;
            }
        }

        basis
    };

    for &node_idx in &topo_order {
        let parent_world = match parent_map.get(&node_idx) {
            Some(&parent_idx) => effective_world[parent_idx],
            None => Matrix4::identity(),
        };

        if !mapped_sl_node_indices.contains(&node_idx) {
            effective_world[node_idx] = parent_world * original_locals[node_idx];
            continue;
        }

        let snapshot_t = match node_worlds_snapshot.get(node_idx) {
            Some(m) => Vector3::new(m[(0, 3)], m[(1, 3)], m[(2, 3)]),
            None => continue,
        };

        let parent_t = Vector3::new(
            parent_world[(0, 3)],
            parent_world[(1, 3)],
            parent_world[(2, 3)],
        );
        let parent_rot_3x3 = extract_rotation_3x3(&parent_world);
        let parent_rot = UnitQuaternion::from_matrix(&parent_rot_3x3);

        let world_offset = snapshot_t - parent_t;
        let local_t = parent_rot.inverse() * world_offset;

        let will_normalize = rotation_normalize_node_indices.contains(&node_idx);

        if let Some(obj) = json["nodes"][node_idx].as_object_mut() {
            obj.remove("matrix");
            obj.insert(
                "translation".to_string(),
                serde_json::json!([local_t.x, local_t.y, local_t.z]),
            );

            obj.insert("scale".to_string(), serde_json::json!([1.0, 1.0, 1.0]));

            if will_normalize {
                obj.insert(
                    "rotation".to_string(),
                    serde_json::json!([0.0, 0.0, 0.0, 1.0]),
                );
            }
        }

        let local_after = node_to_local_matrix(&json["nodes"][node_idx]);
        effective_world[node_idx] = parent_world * local_after;
    }

    log_finger_transforms_for_debug(json);
    node_worlds_snapshot
}

#[allow(dead_code)]
pub(in super::super) fn correct_mesh_vertices_for_bind_pose_change(
    json: &Value,
    bin: &mut [u8],
    old_world_matrices: &[Matrix4<f32>],
) -> Result<()> {
    let node_locals: Vec<Matrix4<f32>> = json["nodes"]
        .as_array()
        .map(|nodes| nodes.iter().map(node_to_local_matrix).collect())
        .unwrap_or_default();
    let parent_map = collect_parent_index_map_from_json(json);
    let new_worlds = compute_node_world_matrices(&node_locals, &parent_map);

    let node_count = old_world_matrices.len().min(new_worlds.len());
    let corrections: Vec<Matrix4<f32>> = (0..node_count)
        .map(|i| {
            let old_inv = old_world_matrices[i]
                .try_inverse()
                .unwrap_or_else(Matrix4::<f32>::identity);
            new_worlds[i] * old_inv
        })
        .collect();

    let skin_count = json["skins"].as_array().map(|s| s.len()).unwrap_or(0);
    let mut corrected_pos_accessors = HashSet::<usize>::new();
    let mut corrected_norm_accessors = HashSet::<usize>::new();

    for skin_idx in 0..skin_count {
        let joints: Vec<usize> = json["skins"][skin_idx]["joints"]
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

        let is_tiny_face_skin = joints.len() <= 4
            && joints.iter().all(|&node_idx| {
                let name = json["nodes"][node_idx]
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                matches!(name, "mHead" | "mEyeLeft" | "mEyeRight")
            });
        if is_tiny_face_skin {
            continue;
        }

        let slot_corrections: Vec<Matrix4<f32>> = joints
            .iter()
            .map(|&ni| corrections.get(ni).copied().unwrap_or_else(Matrix4::identity))
            .collect();

        let any_non_identity = slot_corrections.iter().any(|c| {
            let diff: f32 = (0..4)
                .flat_map(|r| (0..4).map(move |co| (r, co)))
                .map(|(r, co)| {
                    let expected = if r == co { 1.0 } else { 0.0 };
                    (c[(r, co)] - expected).abs()
                })
                .sum();
            diff > 1e-5
        });
        if !any_non_identity {
            continue;
        }

        let prims = collect_primitives_for_skin_with_attributes(json, skin_idx);

        for prim in prims {
            let Some(jnt_meta) = accessor_meta(json, prim.joints_accessor) else {
                continue;
            };
            let Some(wgt_meta) = accessor_meta(json, prim.weights_accessor) else {
                continue;
            };
            if jnt_meta.accessor_type != "VEC4" || wgt_meta.accessor_type != "VEC4" {
                continue;
            }
            if !(jnt_meta.component_type == 5121 || jnt_meta.component_type == 5123) {
                continue;
            }
            if wgt_meta.component_type != 5126 {
                continue;
            }

            let vert_count = jnt_meta.count.min(wgt_meta.count);

            if let Some(pos_acc_idx) = prim.position_accessor {
                if corrected_pos_accessors.insert(pos_acc_idx) {
                    if let Some(pos_meta) = accessor_meta(json, pos_acc_idx) {
                        if pos_meta.component_type == 5126 && pos_meta.accessor_type == "VEC3" {
                            let count = vert_count.min(pos_meta.count);
                            for vi in 0..count {
                                let corr = blend_correction_matrix(
                                    bin,
                                    &jnt_meta,
                                    &wgt_meta,
                                    vi,
                                    &slot_corrections,
                                );
                                let off = pos_meta.base_offset + vi * pos_meta.stride;
                                if off + 12 > bin.len() {
                                    continue;
                                }
                                let px = f32::from_le_bytes([
                                    bin[off],
                                    bin[off + 1],
                                    bin[off + 2],
                                    bin[off + 3],
                                ]);
                                let py = f32::from_le_bytes([
                                    bin[off + 4],
                                    bin[off + 5],
                                    bin[off + 6],
                                    bin[off + 7],
                                ]);
                                let pz = f32::from_le_bytes([
                                    bin[off + 8],
                                    bin[off + 9],
                                    bin[off + 10],
                                    bin[off + 11],
                                ]);
                                let new_p = corr * Vector4::new(px, py, pz, 1.0);
                                bin[off..off + 4].copy_from_slice(&new_p.x.to_le_bytes());
                                bin[off + 4..off + 8].copy_from_slice(&new_p.y.to_le_bytes());
                                bin[off + 8..off + 12].copy_from_slice(&new_p.z.to_le_bytes());
                            }
                        }
                    }
                }
            }

            if let Some(norm_acc_idx) = prim.normal_accessor {
                if corrected_norm_accessors.insert(norm_acc_idx) {
                    if let Some(norm_meta) = accessor_meta(json, norm_acc_idx) {
                        if norm_meta.component_type == 5126 && norm_meta.accessor_type == "VEC3" {
                            let count = vert_count.min(norm_meta.count);
                            for vi in 0..count {
                                let corr = blend_correction_matrix(
                                    bin,
                                    &jnt_meta,
                                    &wgt_meta,
                                    vi,
                                    &slot_corrections,
                                );
                                let off = norm_meta.base_offset + vi * norm_meta.stride;
                                if off + 12 > bin.len() {
                                    continue;
                                }
                                let nx = f32::from_le_bytes([
                                    bin[off],
                                    bin[off + 1],
                                    bin[off + 2],
                                    bin[off + 3],
                                ]);
                                let ny = f32::from_le_bytes([
                                    bin[off + 4],
                                    bin[off + 5],
                                    bin[off + 6],
                                    bin[off + 7],
                                ]);
                                let nz = f32::from_le_bytes([
                                    bin[off + 8],
                                    bin[off + 9],
                                    bin[off + 10],
                                    bin[off + 11],
                                ]);
                                let r00 = corr[(0, 0)];
                                let r01 = corr[(0, 1)];
                                let r02 = corr[(0, 2)];
                                let r10 = corr[(1, 0)];
                                let r11 = corr[(1, 1)];
                                let r12 = corr[(1, 2)];
                                let r20 = corr[(2, 0)];
                                let r21 = corr[(2, 1)];
                                let r22 = corr[(2, 2)];
                                let rnx = r00 * nx + r01 * ny + r02 * nz;
                                let rny = r10 * nx + r11 * ny + r12 * nz;
                                let rnz = r20 * nx + r21 * ny + r22 * nz;
                                let len = (rnx * rnx + rny * rny + rnz * rnz).sqrt();
                                let (rnx, rny, rnz) = if len > 1e-8 {
                                    (rnx / len, rny / len, rnz / len)
                                } else {
                                    (nx, ny, nz)
                                };
                                bin[off..off + 4].copy_from_slice(&rnx.to_le_bytes());
                                bin[off + 4..off + 8].copy_from_slice(&rny.to_le_bytes());
                                bin[off + 8..off + 12].copy_from_slice(&rnz.to_le_bytes());
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[allow(dead_code)]
fn blend_correction_matrix(
    bin: &[u8],
    jnt_meta: &super::super::gltf_utils::AccessorMeta,
    wgt_meta: &super::super::gltf_utils::AccessorMeta,
    vertex: usize,
    slot_corrections: &[Matrix4<f32>],
) -> Matrix4<f32> {
    let mut result = Matrix4::<f32>::zeros();
    let mut total_weight = 0.0f32;

    for lane in 0..4 {
        let slot = read_joint_slot(bin, jnt_meta, vertex, lane).unwrap_or(0) as usize;
        let weight = read_weight_f32(bin, wgt_meta, vertex, lane).unwrap_or(0.0);

        if weight <= 1e-7 {
            continue;
        }

        let corr = slot_corrections
            .get(slot)
            .copied()
            .unwrap_or_else(Matrix4::identity);
        result += weight * corr;
        total_weight += weight;
    }

    if total_weight < 1e-7 {
        return Matrix4::identity();
    }

    result / total_weight
}

#[allow(dead_code)]
struct PrimitiveBindingWithAttributes {
    joints_accessor: usize,
    weights_accessor: usize,
    position_accessor: Option<usize>,
    normal_accessor: Option<usize>,
}

#[allow(dead_code)]
fn collect_primitives_for_skin_with_attributes(
    json: &Value,
    skin_index: usize,
) -> Vec<PrimitiveBindingWithAttributes> {
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

    let mut seen = HashSet::<(usize, usize, Option<usize>, Option<usize>)>::new();
    let mut bindings = Vec::new();

    for node in nodes {
        let Some(node_skin) = node.get("skin").and_then(Value::as_u64).map(|v| v as usize) else {
            continue;
        };
        if node_skin != skin_index {
            continue;
        }
        let Some(mesh_index) = node.get("mesh").and_then(Value::as_u64).map(|v| v as usize) else {
            continue;
        };
        let Some(mesh) = meshes.get(mesh_index) else {
            continue;
        };
        let Some(primitives) = mesh.get("primitives").and_then(Value::as_array) else {
            continue;
        };

        for primitive in primitives {
            let Some(attrs) = primitive.get("attributes").and_then(Value::as_object) else {
                continue;
            };
            let Some(jnt_acc) = attrs
                .get("JOINTS_0")
                .and_then(Value::as_u64)
                .map(|v| v as usize)
            else {
                continue;
            };
            let Some(wgt_acc) = attrs
                .get("WEIGHTS_0")
                .and_then(Value::as_u64)
                .map(|v| v as usize)
            else {
                continue;
            };
            let pos_acc = attrs
                .get("POSITION")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            let norm_acc = attrs
                .get("NORMAL")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            if !seen.insert((jnt_acc, wgt_acc, pos_acc, norm_acc)) {
                continue;
            }
            bindings.push(PrimitiveBindingWithAttributes {
                joints_accessor: jnt_acc,
                weights_accessor: wgt_acc,
                position_accessor: pos_acc,
                normal_accessor: norm_acc,
            });
        }
    }

    bindings
}
