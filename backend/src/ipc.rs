use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    convert::{AnalysisReport, ConversionReport, ConvertOptions, analyze_vrm, convert_vrm_to_gdb},
    notify::send_desktop_notification,
    project::{ProjectSettings, load_project_settings, save_project_settings},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeRequest {
    pub input_path: String,
    pub options: ConvertOptions,
    pub notify_on_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertRequest {
    pub input_path: String,
    pub output_path: String,
    pub options: ConvertOptions,
    pub notify_on_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSettingsRequest {
    pub path: String,
    pub settings: ProjectSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSettingsRequest {
    pub path: String,
}

pub fn analyze_vrm_ipc(request: AnalyzeRequest) -> Result<AnalysisReport, String> {
    let input = PathBuf::from(&request.input_path);
    let report = analyze_vrm(&input, request.options).map_err(|err| err.to_string())?;

    if request.notify_on_complete {
        let _ = send_desktop_notification("vrm2sl", "解析が完了しました");
    }

    Ok(report)
}

pub fn convert_vrm_to_gdb_ipc(request: ConvertRequest) -> Result<ConversionReport, String> {
    let input = PathBuf::from(&request.input_path);
    let output = PathBuf::from(&request.output_path);

    let report = convert_vrm_to_gdb(&input, &output, request.options).map_err(|err| err.to_string())?;

    if request.notify_on_complete {
        let _ = send_desktop_notification("vrm2sl", "変換が完了しました");
    }

    Ok(report)
}

pub fn save_project_settings_ipc(request: SaveSettingsRequest) -> Result<(), String> {
    let path = PathBuf::from(request.path);
    save_project_settings(&path, &request.settings).map_err(|err| err.to_string())
}

pub fn load_project_settings_ipc(request: LoadSettingsRequest) -> Result<ProjectSettings, String> {
    let path = PathBuf::from(request.path);
    load_project_settings(&path).map_err(|err| err.to_string())
}
