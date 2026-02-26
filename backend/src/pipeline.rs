use std::collections::HashMap;

use nalgebra::{Matrix4, UnitQuaternion, Vector3};
use thiserror::Error;

use crate::correction::{
    apply_corrected_rotation, compute_pose_correction, correct_vertex_with_inverse,
    rebuild_inverse_bind_matrix,
};

#[derive(Debug, Clone)]
/// Input payload for correcting a single bone from A-pose to T-pose.
pub struct BoneCorrectionInput {
    /// Current local-space bone rotation before correction.
    pub current_local_rotation: UnitQuaternion<f32>,
    /// Desired local-space rotation representing target T-pose.
    pub target_t_pose_rotation: UnitQuaternion<f32>,
    /// Parent node world transform matrix.
    pub parent_world_matrix: Matrix4<f32>,
    /// Current node local transform matrix.
    pub local_transform_matrix: Matrix4<f32>,
    /// Vertex positions influenced by this bone.
    pub vertices: Vec<Vector3<f32>>,
}

#[derive(Debug, Clone)]
/// Output payload after correcting a single bone and rebuilding bind data.
pub struct BoneCorrectionResult {
    /// Corrected local-space bone rotation.
    pub corrected_local_rotation: UnitQuaternion<f32>,
    /// Vertex positions after inverse visual correction.
    pub corrected_vertices: Vec<Vector3<f32>>,
    /// Rebuilt inverse bind matrix for skinning.
    pub inverse_bind_matrix: Matrix4<f32>,
}

#[derive(Debug, Clone)]
/// Node-level correction input used by batch processing.
pub struct NodeCorrectionInput {
    /// Stable node index from parsed scene data.
    pub node_index: usize,
    /// Node name used for target-map lookup.
    pub node_name: String,
    /// Current local-space node rotation.
    pub current_local_rotation: UnitQuaternion<f32>,
    /// Optional target local rotation; when `None`, rotation is kept as-is.
    pub target_t_pose_rotation: Option<UnitQuaternion<f32>>,
    /// Parent node world transform matrix.
    pub parent_world_matrix: Matrix4<f32>,
    /// Current node local transform matrix.
    pub local_transform_matrix: Matrix4<f32>,
    /// Node-local vertex positions associated with this node.
    pub vertices: Vec<Vector3<f32>>,
}

#[derive(Debug, Clone)]
/// Parsed transform data extracted from a VRM/glTF node.
pub struct ParsedNodeTransform {
    /// Stable node index from parsed scene data.
    pub node_index: usize,
    /// Parsed node name.
    pub node_name: String,
    /// Parsed current local-space rotation.
    pub current_local_rotation: UnitQuaternion<f32>,
    /// Parsed parent world matrix.
    pub parent_world_matrix: Matrix4<f32>,
    /// Parsed local transform matrix.
    pub local_transform_matrix: Matrix4<f32>,
}

#[derive(Debug, Clone)]
/// Parsed geometry data associated with a node index.
pub struct ParsedNodeGeometry {
    /// Node index this geometry belongs to.
    pub node_index: usize,
    /// Parsed vertex positions for the node.
    pub vertices: Vec<Vector3<f32>>,
}

#[derive(Debug, Clone)]
/// Node-level correction output produced by batch processing.
pub struct NodeCorrectionResult {
    /// Node index in parsed scene data.
    pub node_index: usize,
    /// Node name used during correction.
    pub node_name: String,
    /// Output local rotation after optional correction.
    pub corrected_local_rotation: UnitQuaternion<f32>,
    /// Output vertices after optional inverse visual correction.
    pub corrected_vertices: Vec<Vector3<f32>>,
    /// Rebuilt inverse bind matrix for the node.
    pub inverse_bind_matrix: Matrix4<f32>,
    /// True when a T-pose rotation correction was applied.
    pub was_corrected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
/// Error values returned by correction pipeline entry points.
pub enum PipelineError {
    /// Returned when `parent_world_matrix * local_transform_matrix` cannot be
    /// inverted while building an inverse bind matrix.
    #[error("failed to invert bind matrix during {phase} for node {node_index} ({node_name})")]
    NonInvertibleBindMatrix {
        /// Processing phase where the failure occurred.
        phase: &'static str,
        /// Index of the node whose bind matrix could not be inverted.
        node_index: usize,
        /// Name of the node whose bind matrix could not be inverted.
        node_name: String,
    },
}

/// Applies A-pose to T-pose correction for a single bone and regenerates
/// its inverse bind matrix.
///
/// # Arguments
///
/// * `input` - Single-bone correction payload containing transforms and vertices.
///
/// # Returns
///
/// `Some(BoneCorrectionResult)` when the bind matrix is invertible,
/// otherwise `None`.
pub fn correct_bone_to_t_pose(input: BoneCorrectionInput) -> Option<BoneCorrectionResult> {
    let correction =
        compute_pose_correction(input.current_local_rotation, input.target_t_pose_rotation);

    let corrected_local_rotation =
        apply_corrected_rotation(input.current_local_rotation, correction);

    let corrected_vertices = input
        .vertices
        .into_iter()
        .map(|v| correct_vertex_with_inverse(v, correction))
        .collect();

    let inverse_bind_matrix =
        rebuild_inverse_bind_matrix(input.parent_world_matrix, input.local_transform_matrix)?;

    Some(BoneCorrectionResult {
        corrected_local_rotation,
        corrected_vertices,
        inverse_bind_matrix,
    })
}

/// Applies node-wise correction for a parsed skeleton.
///
/// Nodes with `target_t_pose_rotation = Some(..)` are corrected.
/// Nodes with `None` keep their current rotation/vertices and only regenerate
/// inverse bind matrices.
///
/// # Arguments
///
/// * `nodes` - Node correction inputs produced from parsed scene data.
///
/// # Returns
///
/// Corrected node outputs for all inputs in original order.
///
/// # Errors
///
/// Returns `PipelineError::NonInvertibleBindMatrix` when a bind matrix cannot
/// be inverted for any node.
pub fn correct_nodes_to_t_pose(
    nodes: Vec<NodeCorrectionInput>,
) -> Result<Vec<NodeCorrectionResult>, PipelineError> {
    nodes
        .into_iter()
        .map(|node| {
            if let Some(target_t_pose_rotation) = node.target_t_pose_rotation {
                let input = BoneCorrectionInput {
                    current_local_rotation: node.current_local_rotation,
                    target_t_pose_rotation,
                    parent_world_matrix: node.parent_world_matrix,
                    local_transform_matrix: node.local_transform_matrix,
                    vertices: node.vertices,
                };

                let result = correct_bone_to_t_pose(input).ok_or(
                    PipelineError::NonInvertibleBindMatrix {
                        phase: "targeted_node_correction",
                        node_index: node.node_index,
                        node_name: node.node_name.clone(),
                    },
                )?;

                Ok(NodeCorrectionResult {
                    node_index: node.node_index,
                    node_name: node.node_name,
                    corrected_local_rotation: result.corrected_local_rotation,
                    corrected_vertices: result.corrected_vertices,
                    inverse_bind_matrix: result.inverse_bind_matrix,
                    was_corrected: true,
                })
            } else {
                let inverse_bind_matrix = rebuild_inverse_bind_matrix(
                    node.parent_world_matrix,
                    node.local_transform_matrix,
                )
                .ok_or(PipelineError::NonInvertibleBindMatrix {
                    phase: "passthrough_inverse_bind_rebuild",
                    node_index: node.node_index,
                    node_name: node.node_name.clone(),
                })?;

                Ok(NodeCorrectionResult {
                    node_index: node.node_index,
                    node_name: node.node_name,
                    corrected_local_rotation: node.current_local_rotation,
                    corrected_vertices: node.vertices,
                    inverse_bind_matrix,
                    was_corrected: false,
                })
            }
        })
        .collect()
}

/// Builds default target local rotations for SL-style upper-limb T-pose bones.
///
/// This map is intended as a practical starter set for shoulder/arm correction
/// after VRM parsing. Values are local-space target rotations.
///
/// # Returns
///
/// A map from node/bone names to target local-space rotations.
pub fn build_default_upper_limb_t_pose_targets() -> HashMap<String, UnitQuaternion<f32>> {
    let mut targets = HashMap::new();

    targets.insert("mShoulderLeft".to_string(), UnitQuaternion::identity());
    targets.insert("mElbowLeft".to_string(), UnitQuaternion::identity());
    targets.insert("mWristLeft".to_string(), UnitQuaternion::identity());
    targets.insert("mShoulderRight".to_string(), UnitQuaternion::identity());
    targets.insert("mElbowRight".to_string(), UnitQuaternion::identity());
    targets.insert("mWristRight".to_string(), UnitQuaternion::identity());

    targets.insert("leftUpperArm".to_string(), UnitQuaternion::identity());
    targets.insert("leftLowerArm".to_string(), UnitQuaternion::identity());
    targets.insert("leftHand".to_string(), UnitQuaternion::identity());
    targets.insert("rightUpperArm".to_string(), UnitQuaternion::identity());
    targets.insert("rightLowerArm".to_string(), UnitQuaternion::identity());
    targets.insert("rightHand".to_string(), UnitQuaternion::identity());

    targets
}

/// Resolves a node name to an optional target local rotation.
///
/// # Arguments
///
/// * `node_name` - Node or bone name to resolve.
/// * `targets` - Target rotation map keyed by node name.
///
/// # Returns
///
/// `Some(rotation)` when found, otherwise `None`.
pub fn resolve_target_t_pose_rotation(
    node_name: &str,
    targets: &HashMap<String, UnitQuaternion<f32>>,
) -> Option<UnitQuaternion<f32>> {
    targets.get(node_name).cloned()
}

/// Builds correction inputs from parsed node transforms and optional per-node
/// geometry buffers.
///
/// Any node without an entry in `node_geometries` receives an empty vertex list.
///
/// # Arguments
///
/// * `node_transforms` - Parsed transform records for each node.
/// * `node_geometries` - Optional parsed geometry grouped by node index.
/// * `targets` - Target rotation map used to resolve T-pose corrections.
///
/// # Returns
///
/// A list of `NodeCorrectionInput` values ready for batch correction.
pub fn build_node_correction_inputs(
    node_transforms: Vec<ParsedNodeTransform>,
    node_geometries: Vec<ParsedNodeGeometry>,
    targets: &HashMap<String, UnitQuaternion<f32>>,
) -> Vec<NodeCorrectionInput> {
    let geometry_map: HashMap<usize, Vec<Vector3<f32>>> = node_geometries
        .into_iter()
        .map(|geometry| (geometry.node_index, geometry.vertices))
        .collect();

    node_transforms
        .into_iter()
        .map(|transform| NodeCorrectionInput {
            node_index: transform.node_index,
            target_t_pose_rotation: resolve_target_t_pose_rotation(&transform.node_name, targets),
            vertices: geometry_map
                .get(&transform.node_index)
                .cloned()
                .unwrap_or_default(),
            node_name: transform.node_name,
            current_local_rotation: transform.current_local_rotation,
            parent_world_matrix: transform.parent_world_matrix,
            local_transform_matrix: transform.local_transform_matrix,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{Translation3, UnitQuaternion, Vector3};

    #[test]
    fn given_single_bone_when_running_pipeline_then_correction_and_inverse_bind_are_valid() {
        let input = BoneCorrectionInput {
            current_local_rotation: UnitQuaternion::from_euler_angles(0.0, 0.0, -0.4),
            target_t_pose_rotation: UnitQuaternion::identity(),
            parent_world_matrix: Translation3::new(0.0, 1.0, 0.0).to_homogeneous(),
            local_transform_matrix: Translation3::new(2.0, 0.0, 0.0).to_homogeneous(),
            vertices: vec![Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0)],
        };

        let result = correct_bone_to_t_pose(input).expect("pipeline should succeed");

        let q = result.corrected_local_rotation.quaternion();
        assert!(q.w > 0.9999);
        assert!(q.i.abs() < 0.0001);
        assert!(q.j.abs() < 0.0001);
        assert!(q.k.abs() < 0.0001);

        assert_eq!(result.corrected_vertices.len(), 2);

        let bind = Translation3::new(0.0, 1.0, 0.0).to_homogeneous()
            * Translation3::new(2.0, 0.0, 0.0).to_homogeneous();
        let identity = bind * result.inverse_bind_matrix;
        let expected = Matrix4::<f32>::identity();
        assert!((identity - expected).norm() < 0.0001);
    }

    #[test]
    fn given_mixed_nodes_when_running_batch_then_only_targeted_nodes_are_corrected() {
        let nodes = vec![
            NodeCorrectionInput {
                node_index: 0,
                node_name: "mShoulderLeft".to_string(),
                current_local_rotation: UnitQuaternion::from_euler_angles(0.0, 0.0, -0.4),
                target_t_pose_rotation: Some(UnitQuaternion::identity()),
                parent_world_matrix: Translation3::new(0.0, 1.0, 0.0).to_homogeneous(),
                local_transform_matrix: Translation3::new(2.0, 0.0, 0.0).to_homogeneous(),
                vertices: vec![Vector3::new(1.0, 0.0, 0.0)],
            },
            NodeCorrectionInput {
                node_index: 1,
                node_name: "mChest".to_string(),
                current_local_rotation: UnitQuaternion::from_euler_angles(0.0, 0.0, 0.1),
                target_t_pose_rotation: None,
                parent_world_matrix: Translation3::new(0.0, 1.2, 0.0).to_homogeneous(),
                local_transform_matrix: Translation3::new(0.0, 0.3, 0.0).to_homogeneous(),
                vertices: vec![Vector3::new(0.0, 1.0, 0.0)],
            },
        ];

        let results = correct_nodes_to_t_pose(nodes).expect("node loop should succeed");
        assert_eq!(results.len(), 2);

        let shoulder = &results[0];
        assert!(shoulder.was_corrected);
        let q = shoulder.corrected_local_rotation.quaternion();
        assert!(q.w > 0.9999);
        assert!(q.i.abs() < 0.0001);
        assert!(q.j.abs() < 0.0001);
        assert!(q.k.abs() < 0.0001);

        let chest = &results[1];
        assert!(!chest.was_corrected);
        let expected = UnitQuaternion::from_euler_angles(0.0, 0.0, 0.1);
        let delta = chest.corrected_local_rotation.rotation_to(&expected);
        assert!(delta.angle().abs() < 0.0001);
    }

    #[test]
    fn given_default_target_map_when_built_then_upper_limb_bones_exist() {
        let targets = build_default_upper_limb_t_pose_targets();

        assert!(targets.contains_key("mShoulderLeft"));
        assert!(targets.contains_key("mElbowLeft"));
        assert!(targets.contains_key("mWristLeft"));
        assert!(targets.contains_key("mShoulderRight"));
        assert!(targets.contains_key("mElbowRight"));
        assert!(targets.contains_key("mWristRight"));
        assert!(targets.contains_key("leftUpperArm"));
        assert!(targets.contains_key("rightUpperArm"));
    }

    #[test]
    fn given_unknown_bone_when_resolving_target_rotation_then_none_is_returned() {
        let targets = build_default_upper_limb_t_pose_targets();
        let result = resolve_target_t_pose_rotation("mHead", &targets);
        assert!(result.is_none());
    }

    #[test]
    fn given_parsed_nodes_when_building_inputs_then_targets_and_geometry_are_resolved() {
        let targets = build_default_upper_limb_t_pose_targets();
        let node_transforms = vec![
            ParsedNodeTransform {
                node_index: 0,
                node_name: "mShoulderLeft".to_string(),
                current_local_rotation: UnitQuaternion::from_euler_angles(0.0, 0.0, -0.4),
                parent_world_matrix: Translation3::new(0.0, 1.0, 0.0).to_homogeneous(),
                local_transform_matrix: Translation3::new(2.0, 0.0, 0.0).to_homogeneous(),
            },
            ParsedNodeTransform {
                node_index: 1,
                node_name: "mHead".to_string(),
                current_local_rotation: UnitQuaternion::identity(),
                parent_world_matrix: Translation3::new(0.0, 1.5, 0.0).to_homogeneous(),
                local_transform_matrix: Translation3::new(0.0, 0.2, 0.0).to_homogeneous(),
            },
        ];
        let node_geometries = vec![ParsedNodeGeometry {
            node_index: 0,
            vertices: vec![Vector3::new(1.0, 0.0, 0.0)],
        }];

        let inputs = build_node_correction_inputs(node_transforms, node_geometries, &targets);
        assert_eq!(inputs.len(), 2);

        assert!(inputs[0].target_t_pose_rotation.is_some());
        assert_eq!(inputs[0].vertices.len(), 1);

        assert!(inputs[1].target_t_pose_rotation.is_none());
        assert!(inputs[1].vertices.is_empty());
    }

    #[test]
    fn given_non_invertible_bind_error_when_formatted_then_phase_is_included() {
        let error = PipelineError::NonInvertibleBindMatrix {
            phase: "targeted_node_correction",
            node_index: 7,
            node_name: "mShoulderLeft".to_string(),
        };

        let message = error.to_string();
        assert!(message.contains("targeted_node_correction"));
        assert!(message.contains("7"));
        assert!(message.contains("mShoulderLeft"));
    }
}
