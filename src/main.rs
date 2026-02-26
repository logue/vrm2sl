use std::{env, fs, path::PathBuf, process};

use anyhow::{Context, Result, bail};
use vrm2sl::{
    convert::{ConvertOptions, analyze_vrm, convert_vrm_to_gdb},
    project::{ProjectSettings, load_project_settings, save_project_settings},
    texture::ResizeInterpolation,
};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = env::args().skip(1).collect::<Vec<String>>();

    if args.len() < 2 {
        eprintln!(
            "Usage: vrm2sl <input.vrm> <output.gdb> [--target-height <cm>] [--manual-scale <n>] [--resize on|off] [--resize-method bilinear|nearest|bicubic|gaussian|lanczos3] [--report <report.json>] [--analyze-only] [--load-settings <file.json>] [--save-settings <file.json>]"
        );
        process::exit(2);
    }

    let input = PathBuf::from(args.remove(0));
    let output = PathBuf::from(args.remove(0));

    let mut project_settings = ProjectSettings::default();
    project_settings.input_path = Some(input.to_string_lossy().to_string());
    project_settings.output_path = Some(output.to_string_lossy().to_string());

    let mut report_path: Option<PathBuf> = None;
    let mut analyze_only = false;
    let mut save_settings_path: Option<PathBuf> = None;

    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--target-height" => {
                let value = args.get(index + 1).context("--target-height requires a value")?;
                project_settings.target_height_cm = value
                    .parse::<f32>()
                    .with_context(|| format!("invalid --target-height: {value}"))?;
                index += 2;
            }
            "--manual-scale" => {
                let value = args.get(index + 1).context("--manual-scale requires a value")?;
                project_settings.manual_scale = value
                    .parse::<f32>()
                    .with_context(|| format!("invalid --manual-scale: {value}"))?;
                index += 2;
            }
            "--resize" => {
                let value = args.get(index + 1).context("--resize requires on|off")?;
                project_settings.texture_auto_resize = match value.as_str() {
                    "on" => true,
                    "off" => false,
                    _ => bail!("--resize must be on or off"),
                };
                index += 2;
            }
            "--resize-method" => {
                let value = args
                    .get(index + 1)
                    .context("--resize-method requires a value")?;
                project_settings.texture_resize_method = parse_resize_method(value)?;
                index += 2;
            }
            "--report" => {
                let value = args.get(index + 1).context("--report requires a path")?;
                report_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--analyze-only" => {
                analyze_only = true;
                index += 1;
            }
            "--load-settings" => {
                let value = args
                    .get(index + 1)
                    .context("--load-settings requires a path")?;
                project_settings = load_project_settings(&PathBuf::from(value))?;
                project_settings.input_path = Some(input.to_string_lossy().to_string());
                project_settings.output_path = Some(output.to_string_lossy().to_string());
                index += 2;
            }
            "--save-settings" => {
                let value = args
                    .get(index + 1)
                    .context("--save-settings requires a path")?;
                save_settings_path = Some(PathBuf::from(value));
                index += 2;
            }
            unknown => {
                bail!("unknown option: {unknown}");
            }
        }
    }

    let options = ConvertOptions {
        target_height_cm: project_settings.target_height_cm,
        manual_scale: project_settings.manual_scale,
        texture_auto_resize: project_settings.texture_auto_resize,
        texture_resize_method: project_settings.texture_resize_method,
    };

    let analysis = analyze_vrm(&input, options)?;

    if let Some(path) = report_path {
        let json = serde_json::to_string_pretty(&analysis).context("failed to serialize report")?;
        fs::write(&path, json).with_context(|| format!("failed to write report: {}", path.display()))?;
    }

    if analyze_only {
        println!("Model: {}", analysis.model_name);
        println!("Author: {}", analysis.author.unwrap_or_else(|| "Unknown".to_string()));
        println!("Estimated height: {:.2}cm", analysis.estimated_height_cm);
        println!(
            "Meshes: {}, Bones: {}, Vertices: {}, Polygons: {}",
            analysis.mesh_count, analysis.bone_count, analysis.total_vertices, analysis.total_polygons
        );
        println!(
            "Texture fee estimate: {}L$ -> {}L$ ({}%)",
            analysis.fee_estimate.before_linden_dollar,
            analysis.fee_estimate.after_resize_linden_dollar,
            analysis.fee_estimate.reduction_percent
        );
        println!("Issues: {}", analysis.issues.len());
        for issue in analysis.issues {
            println!("- [{:?}] {}", issue.severity, issue.message);
        }

        if let Some(path) = save_settings_path {
            save_project_settings(&path, &project_settings)?;
        }
        return Ok(());
    }

    let report = convert_vrm_to_gdb(&input, &output, options)?;

    println!("Model: {}", report.model_name);
    println!(
        "Height: {:.2}cm -> {:.2}cm (scale {:.4})",
        report.estimated_height_cm, report.target_height_cm, report.computed_scale_factor
    );
    println!(
        "Meshes: {}, Bones: {}, Vertices: {}, Polygons: {}",
        report.mesh_count, report.bone_count, report.total_vertices, report.total_polygons
    );
    println!(
        "Textures: {} (>{}px: {})",
        report.texture_count, 1024, report.texture_over_1024_count
    );
    println!(
        "Texture fee estimate: {}L$ -> {}L$ ({}%)",
        report.fee_estimate.before_linden_dollar,
        report.fee_estimate.after_resize_linden_dollar,
        report.fee_estimate.reduction_percent
    );
    println!("Mapped bones: {}", report.mapped_bones.len());

    if !report.issues.is_empty() {
        println!("Issues: {}", report.issues.len());
        for issue in report.issues {
            println!("- [{:?}] {}", issue.severity, issue.message);
        }
    }

    if let Some(path) = save_settings_path {
        save_project_settings(&path, &project_settings)?;
    }

    Ok(())
}

fn parse_resize_method(value: &str) -> Result<ResizeInterpolation> {
    match value.to_ascii_lowercase().as_str() {
        "nearest" => Ok(ResizeInterpolation::Nearest),
        "bilinear" => Ok(ResizeInterpolation::Bilinear),
        "bicubic" => Ok(ResizeInterpolation::Bicubic),
        "gaussian" => Ok(ResizeInterpolation::Gaussian),
        "lanczos3" => Ok(ResizeInterpolation::Lanczos3),
        _ => bail!("invalid --resize-method: {value}"),
    }
}
