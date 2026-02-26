use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    convert::{AnalysisReport, ConversionReport, ConvertOptions, analyze_vrm, convert_vrm_to_gdb},
    notify::send_desktop_notification,
    project::{ProjectSettings, load_project_settings, save_project_settings},
};

/// IPC payload for analyze-only requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeRequest {
    pub input_path: String,
    pub options: ConvertOptions,
    pub notify_on_complete: bool,
}

/// IPC payload for conversion requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertRequest {
    pub input_path: String,
    pub output_path: String,
    pub options: ConvertOptions,
    pub notify_on_complete: bool,
}

/// IPC payload for saving project settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSettingsRequest {
    pub path: String,
    pub settings: ProjectSettings,
}

/// IPC payload for loading project settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSettingsRequest {
    pub path: String,
}

/// IPC payload for generating a backend-side preview GLB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewRequest {
    pub input_path: String,
    pub options: ConvertOptions,
}

/// Analyze a source model through the IPC boundary.
pub fn analyze_vrm_ipc(request: AnalyzeRequest) -> Result<AnalysisReport, String> {
    let input = PathBuf::from(&request.input_path);
    let report = analyze_vrm(&input, request.options).map_err(|err| err.to_string())?;

    if request.notify_on_complete {
        let _ = send_desktop_notification("vrm2sl", "Analysis completed");
    }

    Ok(report)
}

/// Convert a source model through the IPC boundary.
pub fn convert_vrm_to_gdb_ipc(request: ConvertRequest) -> Result<ConversionReport, String> {
    let input = PathBuf::from(&request.input_path);
    let output = PathBuf::from(&request.output_path);

    let report =
        convert_vrm_to_gdb(&input, &output, request.options).map_err(|err| err.to_string())?;

    if request.notify_on_complete {
        let _ = send_desktop_notification("vrm2sl", "Conversion completed");
    }

    Ok(report)
}

/// Build a preview GLB file through the IPC boundary and return its path.
pub fn build_preview_glb_ipc(request: PreviewRequest) -> Result<String, String> {
    let input = PathBuf::from(&request.input_path);
    let output = create_preview_output_path().map_err(|err| err.to_string())?;

    convert_vrm_to_gdb(&input, &output, request.options).map_err(|err| err.to_string())?;

    Ok(output.to_string_lossy().to_string())
}

/// Save project settings through the IPC boundary.
pub fn save_project_settings_ipc(request: SaveSettingsRequest) -> Result<(), String> {
    let path = PathBuf::from(request.path);
    save_project_settings(&path, &request.settings).map_err(|err| err.to_string())
}

/// Load project settings through the IPC boundary.
pub fn load_project_settings_ipc(request: LoadSettingsRequest) -> Result<ProjectSettings, String> {
    let path = PathBuf::from(request.path);
    load_project_settings(&path).map_err(|err| err.to_string())
}

fn create_preview_output_path() -> anyhow::Result<PathBuf> {
    let mut dir = std::env::temp_dir();
    dir.push("vrm2sl-preview");
    fs::create_dir_all(&dir)?;

    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let file_name = format!("preview-{}-{}.glb", std::process::id(), timestamp);

    Ok(dir.join(file_name))
}
