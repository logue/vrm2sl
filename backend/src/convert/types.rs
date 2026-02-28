use serde::{Deserialize, Serialize};

use crate::texture::ResizeInterpolation;

// ─── Bone name constants ──────────────────────────────────────────────────────

/// Required humanoid source bone names expected in VRoid/VRM input.
pub(super) const REQUIRED_BONES: [&str; 17] = [
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
pub(super) const BONE_MAP: [(&str, &str); 19] = [
    ("hips", "mPelvis"),
    ("spine", "mTorso"),
    ("chest", "mChest"),
    ("neck", "mNeck"),
    ("head", "mHead"),
    ("leftShoulder", "mCollarLeft"),
    ("leftUpperArm", "mShoulderLeft"),
    ("leftLowerArm", "mElbowLeft"),
    ("leftHand", "mWristLeft"),
    ("rightShoulder", "mCollarRight"),
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
pub(super) const BENTO_BONE_MAP: [(&str, &str); 33] = [
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
///
/// **Fallback** relations (`chest → leftUpperArm`, `chest → rightUpperArm`) are
/// listed first.  When the VRM model also contains `leftShoulder` /
/// `rightShoulder`, the more specific **refinement** relations
/// (`chest → leftShoulder → leftUpperArm`) override the fallback via the
/// deduplication logic in `reconstruct_sl_core_hierarchy`.
pub(super) const CORE_HIERARCHY_RELATIONS: [(&str, &str); 20] = [
    // ─ fallback (used when leftShoulder / rightShoulder are absent) ───
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
    // ─ refinement: collar/shoulder chain (overrides fallback when present) ─
    ("chest", "leftShoulder"),
    ("leftShoulder", "leftUpperArm"),
    ("chest", "rightShoulder"),
    ("rightShoulder", "rightUpperArm"),
];

/// Optional hierarchy edges for Bento extension bones.
pub(super) const BENTO_HIERARCHY_RELATIONS: [(&str, &str); 33] = [
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

// ─── Public types ─────────────────────────────────────────────────────────────

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
