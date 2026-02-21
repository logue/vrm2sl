use nalgebra::{Matrix4, Point3, UnitQuaternion, Vector3};

/// Computes a correction quaternion that moves a bone from the current local
/// pose to the target local T-pose rotation.
///
/// # Arguments
///
/// * `current_pose` - Current local-space rotation of the bone.
/// * `target_t_pose` - Desired local-space rotation for the T-pose.
///
/// # Returns
///
/// A quaternion that can be multiplied with `current_pose` to obtain the
/// target rotation.
pub fn compute_pose_correction(
    current_pose: UnitQuaternion<f32>,
    target_t_pose: UnitQuaternion<f32>,
) -> UnitQuaternion<f32> {
    target_t_pose * current_pose.inverse()
}

/// Applies a correction quaternion to the current local rotation.
///
/// # Arguments
///
/// * `current_pose` - Current local-space rotation.
/// * `correction` - Correction quaternion from A-pose to T-pose.
///
/// # Returns
///
/// Corrected local-space rotation.
pub fn apply_corrected_rotation(
    current_pose: UnitQuaternion<f32>,
    correction: UnitQuaternion<f32>,
) -> UnitQuaternion<f32> {
    correction * current_pose
}

/// Applies the inverse correction to a vertex so mesh appearance stays stable
/// while the bone rotation is corrected.
///
/// # Arguments
///
/// * `vertex` - Vertex position to be visually compensated.
/// * `correction` - Bone correction quaternion applied to the skeleton.
///
/// # Returns
///
/// Vertex position transformed by the inverse correction.
pub fn correct_vertex_with_inverse(
    vertex: Vector3<f32>,
    correction: UnitQuaternion<f32>,
) -> Vector3<f32> {
    correction.inverse_transform_vector(&vertex)
}

/// Applies a correction matrix directly to a vertex position.
///
/// # Arguments
///
/// * `vertex` - Vertex position to transform.
/// * `correction_matrix` - 4x4 transform matrix applied to the vertex.
///
/// # Returns
///
/// Transformed vertex position.
pub fn correct_vertex_with_matrix(
    vertex: Vector3<f32>,
    correction_matrix: Matrix4<f32>,
) -> Vector3<f32> {
    let p = Point3::from(vertex);
    let corrected = correction_matrix.transform_point(&p);
    corrected.coords
}

/// Rebuilds an inverse bind matrix from parent/world and local transforms.
/// Returns `None` when the bind matrix is not invertible.
///
/// # Arguments
///
/// * `parent_world` - Parent node world transform matrix.
/// * `local_transform` - Current node local transform matrix.
///
/// # Returns
///
/// `Some(inverse_bind_matrix)` when invertible, otherwise `None`.
pub fn rebuild_inverse_bind_matrix(
    parent_world: Matrix4<f32>,
    local_transform: Matrix4<f32>,
) -> Option<Matrix4<f32>> {
    let bind_matrix = parent_world * local_transform;
    bind_matrix.try_inverse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{Translation3, UnitQuaternion, Vector3};

    #[test]
    fn given_a_pose_when_applying_correction_then_rotation_matches_target() {
        let current = UnitQuaternion::from_euler_angles(0.0, 0.0, -0.4);
        let target = UnitQuaternion::identity();
        let correction = compute_pose_correction(current, target);

        let corrected = apply_corrected_rotation(current, correction);
        let q = corrected.quaternion();
        assert!(q.w > 0.9999);
        assert!(q.i.abs() < 0.0001);
        assert!(q.j.abs() < 0.0001);
        assert!(q.k.abs() < 0.0001);
    }

    #[test]
    fn given_deformed_vertex_when_applying_inverse_correction_then_shape_is_preserved() {
        let current = UnitQuaternion::from_euler_angles(0.0, 0.0, -0.4);
        let target = UnitQuaternion::identity();
        let correction = compute_pose_correction(current, target);

        let v = Vector3::new(1.0, 0.0, 0.0);
        let deformed = correction.transform_vector(&v);
        let restored = correct_vertex_with_inverse(deformed, correction);

        assert!((restored - v).norm() < 0.0001);
    }

    #[test]
    fn given_bind_transforms_when_rebuilding_inverse_bind_then_identity_is_recovered() {
        let parent_world = Translation3::new(0.0, 1.0, 0.0).to_homogeneous();
        let local_transform = Translation3::new(2.0, 0.0, 0.0).to_homogeneous();

        let inverse = rebuild_inverse_bind_matrix(parent_world, local_transform)
            .expect("inverse bind matrix should be invertible");

        let bind = parent_world * local_transform;
        let identity = bind * inverse;
        let expected = nalgebra::Matrix4::<f32>::identity();

        assert!((identity - expected).norm() < 0.0001);
    }
}
