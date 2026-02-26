use std::{env, path::PathBuf, process};

use vrm2sl::convert::{ConvertOptions, convert_vrm_to_gdb};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: vrm2sl <input.vrm> <output.gdb>");
        process::exit(2);
    }

    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);

    let report = convert_vrm_to_gdb(&input, &output, ConvertOptions::default())?;

    println!("Model: {}", report.model_name);
    println!(
        "Height: {:.2}cm -> {:.2}cm (scale {:.4})",
        report.estimated_height_cm, report.target_height_cm, report.computed_scale_factor
    );
    println!("Meshes: {}, Bones: {}", report.mesh_count, report.bone_count);
    println!(
        "Textures: {} (>{}px: {})",
        report.texture_count, 1024, report.texture_over_1024_count
    );
    println!("Mapped bones: {}", report.mapped_bones.len());

    Ok(())
}
