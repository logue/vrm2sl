use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::texture::ResizeInterpolation;

/// Blink behavior settings used by lightweight face controls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlinkSettings {
    pub enabled: bool,
    pub interval_sec: f32,
    pub close_duration_sec: f32,
    pub wink_enabled: bool,
}

impl Default for BlinkSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_sec: 4.0,
            close_duration_sec: 0.15,
            wink_enabled: true,
        }
    }
}

/// Lip-sync behavior settings used by lightweight face controls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LipSyncSettings {
    pub enabled: bool,
    pub mode: String,
    pub open_angle: f32,
    pub speed: f32,
}

impl Default for LipSyncSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: "chat".to_string(),
            open_angle: 0.5,
            speed: 0.5,
        }
    }
}

/// Eye-tracking behavior settings for preview/control configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EyeTrackingSettings {
    pub camera_follow: bool,
    pub random_look: bool,
    pub vertical_range_deg: f32,
    pub horizontal_range_deg: f32,
    pub speed: f32,
}

impl Default for EyeTrackingSettings {
    fn default() -> Self {
        Self {
            camera_follow: true,
            random_look: true,
            vertical_range_deg: 25.0,
            horizontal_range_deg: 40.0,
            speed: 0.5,
        }
    }
}

/// Grouped face control settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceSettings {
    pub blink: BlinkSettings,
    pub lip_sync: LipSyncSettings,
    pub eye_tracking: EyeTrackingSettings,
}

impl Default for FaceSettings {
    fn default() -> Self {
        Self {
            blink: BlinkSettings::default(),
            lip_sync: LipSyncSettings::default(),
            eye_tracking: EyeTrackingSettings::default(),
        }
    }
}

/// Finger test/control settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerSettings {
    pub enabled: bool,
    pub test_pose: String,
}

impl Default for FingerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            test_pose: "open".to_string(),
        }
    }
}

/// Persisted project settings used by CLI/UI workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub input_path: Option<String>,
    pub output_path: Option<String>,
    pub target_height_cm: f32,
    pub manual_scale: f32,
    pub texture_auto_resize: bool,
    pub texture_resize_method: ResizeInterpolation,
    pub face: FaceSettings,
    pub fingers: FingerSettings,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            input_path: None,
            output_path: None,
            target_height_cm: 200.0,
            manual_scale: 1.0,
            texture_auto_resize: true,
            texture_resize_method: ResizeInterpolation::Bilinear,
            face: FaceSettings::default(),
            fingers: FingerSettings::default(),
        }
    }
}

/// Save project settings to a JSON file.
pub fn save_project_settings(path: &Path, settings: &ProjectSettings) -> Result<()> {
    let content = serde_json::to_string_pretty(settings)
        .context("failed to serialize project settings as JSON")?;
    fs::write(path, content)
        .with_context(|| format!("failed to save project settings: {}", path.display()))?;
    Ok(())
}

/// Load project settings from a JSON file.
pub fn load_project_settings(path: &Path) -> Result<ProjectSettings> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to load project settings: {}", path.display()))?;
    let settings: ProjectSettings =
        serde_json::from_str(&content).context("failed to parse project settings JSON")?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_default_settings_when_serialize_then_json_contains_target_height() {
        let settings = ProjectSettings::default();
        let json = serde_json::to_string(&settings).expect("serialize settings");
        assert!(json.contains("target_height_cm"));
    }
}
