use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use nalgebra::{Matrix4, Vector3, Vector4};
use serde_json::Value;

use super::gltf_utils::{
    accessor_meta, collect_node_name_set_from_json, collect_parent_index_map_from_json,
    compute_node_world_matrices, node_to_local_matrix, read_joint_slot, read_weight_f32,
    set_node_local_matrix, write_mat4_f32_le,
};
use super::types::{
    BENTO_BONE_MAP, BENTO_HIERARCHY_RELATIONS, BONE_MAP, CORE_HIERARCHY_RELATIONS, ValidationIssue,
};

// ─── Bone renaming ────────────────────────────────────────────────────────────

/// Rename known bones according to the VRM→SL mapping table.
pub(super) fn rename_bones(json: &mut Value, humanoid_bone_nodes: &HashMap<String, usize>) {
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

// ─── Hierarchy reconstruction ─────────────────────────────────────────────────

/// Reconstruct core humanoid hierarchy toward SL-compatible parent-child links.
pub(super) fn reconstruct_sl_core_hierarchy(
    json: &mut Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
) {
    let original_node_locals: Vec<Matrix4<f32>> = json
        .get("nodes")
        .and_then(Value::as_array)
        .map(|nodes| nodes.iter().map(node_to_local_matrix).collect())
        .unwrap_or_default();
    let original_parent_map = collect_parent_index_map_from_json(json);
    let original_node_worlds =
        compute_node_world_matrices(&original_node_locals, &original_parent_map);

    let planned_links: Vec<(usize, usize)> = {
        // Build a child→parent map; later entries (refinement) override
        // earlier entries (fallback), so e.g. when leftShoulder exists the
        // chain becomes chest → leftShoulder → leftUpperArm instead of the
        // fallback chest → leftUpperArm.
        let mut child_to_parent: HashMap<usize, usize> = HashMap::new();
        for (parent, child) in CORE_HIERARCHY_RELATIONS
            .iter()
            .chain(BENTO_HIERARCHY_RELATIONS.iter())
        {
            let Some(parent_index) = humanoid_bone_nodes.get(*parent).copied() else {
                continue;
            };
            let Some(child_index) = humanoid_bone_nodes.get(*child).copied() else {
                continue;
            };
            if parent_index == child_index {
                continue;
            }
            child_to_parent.insert(child_index, parent_index);
        }
        child_to_parent
            .into_iter()
            .map(|(child, parent)| (parent, child))
            .collect()
    };

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

// ─── Bone precondition validation ─────────────────────────────────────────────

/// Validate that required source bones point to valid node indices before conversion.
pub(super) fn validate_bone_conversion_preconditions(
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
                    severity: super::types::Severity::Error,
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
pub(super) fn ensure_target_bones_exist_after_rename(
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

// ─── Rotation normalisation ───────────────────────────────────────────────────

/// Normalize the local rotation of every SL-mapped bone to identity while
/// preserving the bone's world-space position.
///
/// Second Life reads joint bind-positions from the inverse-bind-matrix
/// (4th column) and then applies its **own** default (identity) orientations
/// when deforming the mesh. Any non-identity local rotation baked into the
/// glTF node hierarchy will therefore cause incorrect deformation because the
/// IBM accounts for the rotation but SL does not re-apply it.
///
/// Bones are processed in topological (parent-before-child) order so that each
/// child uses the already-corrected parent world position when computing its
/// own local translation.
///
/// Returns the **pre-normalization** node world matrices so that the caller can
/// correct mesh vertex positions for the bind-pose change.
pub(super) fn normalize_sl_bone_rotations(
    json: &mut Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
) -> Vec<Matrix4<f32>> {
    let sl_node_indices: HashSet<usize> = BONE_MAP
        .iter()
        .chain(BENTO_BONE_MAP.iter())
        .filter_map(|(vrm_name, _)| humanoid_bone_nodes.get(*vrm_name).copied())
        .collect();

    let node_locals: Vec<Matrix4<f32>> = json["nodes"]
        .as_array()
        .map(|nodes| nodes.iter().map(node_to_local_matrix).collect())
        .unwrap_or_default();
    let parent_map = collect_parent_index_map_from_json(json);
    let node_worlds_snapshot = compute_node_world_matrices(&node_locals, &parent_map);

    let node_count = json["nodes"].as_array().map(|a| a.len()).unwrap_or(0);

    // BFS topological order (parents before children).
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

    let mut effective_world_t: Vec<Vector3<f32>> = node_worlds_snapshot
        .iter()
        .map(|m| Vector3::new(m[(0, 3)], m[(1, 3)], m[(2, 3)]))
        .collect();
    while effective_world_t.len() < node_count {
        effective_world_t.push(Vector3::zeros());
    }

    for &node_idx in &topo_order {
        if !sl_node_indices.contains(&node_idx) {
            continue;
        }

        let snapshot_t = match node_worlds_snapshot.get(node_idx) {
            Some(m) => Vector3::new(m[(0, 3)], m[(1, 3)], m[(2, 3)]),
            None => continue,
        };

        let parent_effective_t = match parent_map.get(&node_idx) {
            Some(&parent_idx) => effective_world_t[parent_idx],
            None => Vector3::zeros(),
        };

        let local_t = snapshot_t - parent_effective_t;
        effective_world_t[node_idx] = parent_effective_t + local_t;

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
            // Force identity scale: Second Life does not use bone scale and
            // any residual non-unit scale (even near-identity values like
            // 0.9999998) will cause IBM/joint-position mismatches that
            // manifest as mesh twisting or offset bones.
            obj.insert("scale".to_string(), serde_json::json!([1.0, 1.0, 1.0]));
        }
    }

    node_worlds_snapshot
}

// ─── Bind-pose vertex correction ──────────────────────────────────────────────

/// Correct mesh vertex positions and normals after bone rotations have been
/// normalised to identity.
///
/// When `normalize_sl_bone_rotations` removes bone rotations while preserving
/// world-space translations, the *bind pose* changes: the bones now occupy the
/// same positions but with different orientations. Mesh vertices that were
/// authored to wrap around the original rotated bones will be misaligned unless
/// they are counter-rotated into the new (identity-rotation) bind pose.
///
/// This is the programmatic equivalent of Blender's **Apply Rotation and Scale**
/// on an armature: all rest-rotations are baked into the mesh geometry so the
/// mesh stays visually identical while the bone orientations become identity.
///
/// The per-vertex correction is a standard *change-of-bind-pose* operation:
///
///   `new_pos = Σ wᵢ · (W′ᵢ · W⁻¹ᵢ) · old_pos`
///
/// where `Wᵢ` / `W′ᵢ` are the old / new world matrices for joint *i* and `wᵢ`
/// is the vertex weight for that joint.  Normals are corrected with the
/// upper-left 3×3 of the same blended matrix and re-normalised.
pub(super) fn correct_mesh_vertices_for_bind_pose_change(
    json: &Value,
    bin: &mut [u8],
    old_world_matrices: &[Matrix4<f32>],
) -> Result<()> {
    // Compute post-normalization world matrices.
    let node_locals: Vec<Matrix4<f32>> = json["nodes"]
        .as_array()
        .map(|nodes| nodes.iter().map(node_to_local_matrix).collect())
        .unwrap_or_default();
    let parent_map = collect_parent_index_map_from_json(json);
    let new_worlds = compute_node_world_matrices(&node_locals, &parent_map);

    // Per-node correction: C[i] = new_world[i] · inverse(old_world[i])
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

        // Per-slot correction matrix.
        let slot_corrections: Vec<Matrix4<f32>> = joints
            .iter()
            .map(|&ni| {
                corrections
                    .get(ni)
                    .copied()
                    .unwrap_or_else(Matrix4::identity)
            })
            .collect();

        // Quick check: skip this skin if all corrections are identity.
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

            // ── Correct POSITION ──────────────────────────────────────────
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

            // ── Correct NORMAL ────────────────────────────────────────────
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
                                // Use upper-left 3×3 for normals.
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

/// Compute the weighted correction matrix for a single vertex.
fn blend_correction_matrix(
    bin: &[u8],
    jnt_meta: &super::gltf_utils::AccessorMeta,
    wgt_meta: &super::gltf_utils::AccessorMeta,
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

    // Fallback to identity if no effective weight.
    if total_weight < 1e-7 {
        return Matrix4::identity();
    }

    result
}

/// Primitive attribute info for bind-pose correction.
struct PrimitiveBindingWithAttributes {
    joints_accessor: usize,
    weights_accessor: usize,
    position_accessor: Option<usize>,
    normal_accessor: Option<usize>,
}

/// Collect primitive attribute accessors for all primitives bound to a skin.
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

    let mut seen = HashSet::<(usize, usize)>::new();
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
            if !seen.insert((jnt_acc, wgt_acc)) {
                continue;
            }
            let pos_acc = attrs
                .get("POSITION")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            let norm_acc = attrs
                .get("NORMAL")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
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

// ─── Scene root promotion ─────────────────────────────────────────────────────

/// Clean up wrapper nodes above `mPelvis` in the skeleton hierarchy.
///
/// VRM files commonly have the structure:
///   scene → Root (node 0, sometimes with a non-identity transform)
///               └─ Armature / J_Root
///                      └─ mPelvis → …
///
/// Any non-identity ancestor transform above mPelvis would shift every joint's
/// world matrix, causing deformed or offset limbs in Second Life.
///
/// This function:
/// 1. Keeps the **topmost** non-SL ancestor node ("Root") and resets it to
///    identity so the SL viewer never applies an unwanted offset.
/// 2. Collapses intermediate nodes between that root and mPelvis.
/// 3. Recomputes mPelvis's local translation to match its original world
///    position (now relative to the identity root).
/// 4. Promotes orphaned non-skeleton children (mesh nodes) to the scene.
///
/// Returns the kept identity-root node index (for use as `skin.skeleton`) or
/// `None` if no wrapper ancestors existed.
pub(super) fn promote_pelvis_to_scene_root(
    json: &mut Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
) -> Option<usize> {
    let Some(pelvis_index) = humanoid_bone_nodes.get("hips").copied() else {
        return None;
    };

    let parent_map = collect_parent_index_map_from_json(json);

    // Collect non-SL ancestors from mPelvis up toward the scene root.
    // ancestors[0] = direct parent of mPelvis,
    // ancestors[last] = topmost non-SL ancestor (closest to scene root).
    let mut ancestors: Vec<usize> = Vec::new();
    let mut current = pelvis_index;
    while let Some(&parent_idx) = parent_map.get(&current) {
        let parent_name = json["nodes"][parent_idx]
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let is_sl_bone = parent_name.starts_with('m')
            || humanoid_bone_nodes.values().any(|&idx| idx == parent_idx);
        if is_sl_bone {
            break;
        }
        ancestors.push(parent_idx);
        current = parent_idx;
    }

    if ancestors.is_empty() {
        return None;
    }

    // Compute mPelvis's world position before any modifications.
    let node_locals: Vec<Matrix4<f32>> = json["nodes"]
        .as_array()
        .map(|nodes| nodes.iter().map(node_to_local_matrix).collect())
        .unwrap_or_default();
    let world_matrices = compute_node_world_matrices(&node_locals, &parent_map);

    let pelvis_world = world_matrices
        .get(pelvis_index)
        .copied()
        .unwrap_or_else(Matrix4::<f32>::identity);
    let pelvis_world_t = Vector3::new(
        pelvis_world[(0, 3)],
        pelvis_world[(1, 3)],
        pelvis_world[(2, 3)],
    );

    // The topmost ancestor becomes the identity skeleton root.
    let identity_root_index = *ancestors.last().unwrap();
    // All ancestors except the topmost are "intermediates" to collapse.
    let intermediates: HashSet<usize> = ancestors
        .iter()
        .copied()
        .filter(|&idx| idx != identity_root_index)
        .collect();

    // ── 1. Reset identity root to a pure identity transform. ──────────────
    if let Some(obj) = json["nodes"][identity_root_index].as_object_mut() {
        obj.remove("matrix");
        obj.insert(
            "translation".to_string(),
            serde_json::json!([0.0, 0.0, 0.0]),
        );
        obj.insert(
            "rotation".to_string(),
            serde_json::json!([0.0, 0.0, 0.0, 1.0]),
        );
        obj.insert("scale".to_string(), serde_json::json!([1.0, 1.0, 1.0]));
    }

    // ── 2. Remove mPelvis from its current parent's children list. ────────
    if let Some(direct_parent) = parent_map.get(&pelvis_index).copied() {
        if let Some(dp_node) = json["nodes"][direct_parent].as_object_mut() {
            if let Some(Value::Array(children)) = dp_node.get_mut("children") {
                children.retain(|v| v.as_u64().map(|n| n as usize) != Some(pelvis_index));
            }
        }
    }

    // ── 3. Remove intermediates from their parents; gather orphans. ───────
    let mut nodes_to_promote: Vec<usize> = Vec::new();
    for &ancestor_idx in &ancestors {
        let children_snapshot: Vec<usize> = json["nodes"][ancestor_idx]
            .get("children")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();
        for child_idx in children_snapshot {
            if child_idx != pelvis_index
                && child_idx != identity_root_index
                && !intermediates.contains(&child_idx)
            {
                nodes_to_promote.push(child_idx);
            }
        }
    }
    // Clear intermediate nodes' children lists.
    for &inter_idx in &intermediates {
        if let Some(obj) = json["nodes"][inter_idx].as_object_mut() {
            obj.remove("children");
        }
    }

    // ── 4. Set identity root's children to just mPelvis. ─────────────────
    if let Some(obj) = json["nodes"][identity_root_index].as_object_mut() {
        obj.insert(
            "children".to_string(),
            serde_json::json!([pelvis_index as u64]),
        );
    }

    // ── 5. Update mPelvis with its world-space position. ─────────────────
    //    Since the parent (identity root) has identity transform, the local
    //    translation equals the world translation.
    if let Some(node_obj) = json["nodes"][pelvis_index].as_object_mut() {
        node_obj.remove("matrix");
        node_obj.insert(
            "translation".to_string(),
            serde_json::json!([pelvis_world_t.x, pelvis_world_t.y, pelvis_world_t.z]),
        );
        if !node_obj.contains_key("rotation") {
            node_obj.insert(
                "rotation".to_string(),
                serde_json::json!([0.0, 0.0, 0.0, 1.0]),
            );
        }
    }

    // ── 6. Update the scene root list. ───────────────────────────────────
    //    Remove intermediates from the scene; ensure identity root is present;
    //    promote orphaned mesh nodes.
    if let Some(scene) = json
        .get_mut("scenes")
        .and_then(Value::as_array_mut)
        .and_then(|s| s.first_mut())
    {
        if let Some(Value::Array(scene_nodes)) = scene.get_mut("nodes") {
            // Remove intermediates and the pelvis itself from the scene
            // (pelvis is now under identity root, not a direct scene child).
            scene_nodes.retain(|v| {
                v.as_u64()
                    .map(|n| {
                        let idx = n as usize;
                        !intermediates.contains(&idx) && idx != pelvis_index
                    })
                    .unwrap_or(true)
            });

            // Ensure identity root is in the scene.
            let root_val = Value::from(identity_root_index as u64);
            if !scene_nodes.iter().any(|v| v == &root_val) {
                scene_nodes.push(root_val);
            }

            // Promote orphaned non-skeleton children to the scene.
            for promote_idx in nodes_to_promote {
                let val = Value::from(promote_idx as u64);
                if !scene_nodes.iter().any(|v| v == &val) {
                    scene_nodes.push(val);
                }
            }
        }
    }

    Some(identity_root_index)
}

// ─── skin.skeleton ────────────────────────────────────────────────────────────

/// Set `skin.skeleton` for every skin to the skeleton root node.
///
/// The glTF spec defines `skin.skeleton` as "the index of the node used as a
/// skeleton root."  It does NOT require the skeleton root to be listed in the
/// skin's `joints` array.
///
/// When an identity-root node exists above mPelvis, we point every skin's
/// `skeleton` to that node.  Because the identity root has a pure identity
/// transform, the SL viewer (and other importers that multiply the skeleton
/// root's transform into the skinning equation) will not inject any unwanted
/// offset.  All joints still resolve to their correct world positions through
/// the normal parent-chain traversal.
///
/// When no identity root is available (i.e. `promote_pelvis_to_scene_root`
/// found no wrapper ancestors) we fall back to mPelvis itself.
pub(super) fn set_skin_skeleton_root(
    json: &mut Value,
    humanoid_bone_nodes: &HashMap<String, usize>,
    identity_root: Option<usize>,
) {
    let skeleton_index = identity_root.or_else(|| humanoid_bone_nodes.get("hips").copied());

    let skins = match json["skins"].as_array_mut() {
        Some(s) => s,
        None => return,
    };

    for skin in skins.iter_mut() {
        if let Some(idx) = skeleton_index {
            skin["skeleton"] = Value::Number(idx.into());
            continue;
        }

        // Fallback (no hips bone found): use the first joint as skeleton root.
        let joints: Vec<usize> = skin["joints"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();

        if let Some(&first) = joints.first() {
            skin["skeleton"] = Value::Number(first.into());
        }
    }
}

// ─── Inverse bind matrix regeneration ────────────────────────────────────────

/// Rebuild inverse bind matrices from current node transforms and write them
/// back to the binary buffer for all skins that have writable MAT4 float accessors.
pub(super) fn regenerate_inverse_bind_matrices(json: &mut Value, bin: &mut [u8]) -> Result<()> {
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
