use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result, bail};
use gltf::{Document, binary::Glb, import};
use nalgebra::Vector3;
use serde::Serialize;
use serde_json::Value;

use crate::texture::ResizeInterpolation;

const REQUIRED_BONES: [&str; 17] = [
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

const BONE_MAP: [(&str, &str); 17] = [
    ("hips", "mPelvis"),
    ("spine", "mTorso"),
    ("chest", "mChest"),
    ("neck", "mNeck"),
    ("head", "mHead"),
    ("leftUpperArm", "mShoulderLeft"),
    ("leftLowerArm", "mElbowLeft"),
    ("leftHand", "mWristLeft"),
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

#[derive(Debug, Clone, Copy)]
pub struct ConvertOptions {
    pub target_height_cm: f32,
    pub manual_scale: f32,
    pub texture_auto_resize: bool,
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

#[derive(Debug, Clone, Serialize)]
pub struct ConversionReport {
    pub model_name: String,
    pub author: Option<String>,
    pub estimated_height_cm: f32,
    pub target_height_cm: f32,
    pub computed_scale_factor: f32,
    pub bone_count: usize,
    pub mesh_count: usize,
    pub mapped_bones: Vec<(String, String)>,
    pub texture_count: usize,
    pub texture_over_1024_count: usize,
}

pub fn convert_vrm_to_gdb(
    input_path: &Path,
    output_path: &Path,
    options: ConvertOptions,
) -> Result<ConversionReport> {
    let input_bytes = fs::read(input_path)
        .with_context(|| format!("failed to read input file: {}", input_path.display()))?;
    let input_glb = Glb::from_slice(&input_bytes).context("input VRM is not a GLB container")?;
    let input_json: Value = serde_json::from_slice(input_glb.json.as_ref())
        .context("failed to parse glTF JSON chunk from VRM")?;

    let (document, buffers, images) = import(input_path)
        .with_context(|| format!("failed to read VRM/glTF: {}", input_path.display()))?;

    validate_vroid_model(&input_json)?;

    let node_names = collect_node_names(&document);
    ensure_required_bones_exist(&node_names)?;

    let mapped_bones = collect_mapped_bones(&node_names);
    let estimated_height_cm = estimate_height_cm(&document, &buffers).unwrap_or(170.0);
    let computed_scale_factor = if estimated_height_cm > 0.0 {
        (options.target_height_cm / estimated_height_cm) * options.manual_scale
    } else {
        options.manual_scale
    };

    transform_and_write_glb(input_path, output_path, computed_scale_factor)?;

    let texture_over_1024_count = images
        .iter()
        .filter(|image| image.width > 1024 || image.height > 1024)
        .count();

    Ok(ConversionReport {
        model_name: extract_model_name(&input_json)
            .unwrap_or_else(|| input_path.to_string_lossy().to_string()),
        author: extract_author(&input_json),
        estimated_height_cm,
        target_height_cm: options.target_height_cm,
        computed_scale_factor,
        bone_count: node_names.len(),
        mesh_count: document.meshes().count(),
        mapped_bones,
        texture_count: images.len(),
        texture_over_1024_count,
    })
}

fn validate_vroid_model(json: &Value) -> Result<()> {
    let generator = json
        .get("asset")
        .and_then(|asset| asset.get("generator"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    let vrm_extension_present = json
        .get("extensions")
        .and_then(Value::as_object)
        .map(|ext| {
            ext.keys()
                .any(|key| key.to_ascii_lowercase().contains("vrm"))
        })
        .unwrap_or(false);

    if generator.to_ascii_lowercase().contains("vroid") {
        return Ok(());
    }

    if vrm_extension_present {
        return Ok(());
    }

    bail!("[ERROR] VRoid Studio標準のVRMのみサポートしています")
}

fn collect_node_names(document: &Document) -> HashSet<String> {
    document
        .nodes()
        .filter_map(|node| node.name().map(ToOwned::to_owned))
        .collect()
}

fn ensure_required_bones_exist(node_names: &HashSet<String>) -> Result<()> {
    if let Some(missing) = REQUIRED_BONES
        .iter()
        .find(|bone_name| !node_names.contains(**bone_name))
    {
        bail!("[ERROR] 必須ボーン {} が見つかりません", missing);
    }

    Ok(())
}

fn collect_mapped_bones(node_names: &HashSet<String>) -> Vec<(String, String)> {
    BONE_MAP
        .iter()
        .filter(|(source, _)| node_names.contains(*source))
        .map(|(source, target)| (source.to_string(), target.to_string()))
        .collect()
}

fn estimate_height_cm(document: &Document, buffers: &[gltf::buffer::Data]) -> Option<f32> {
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|b| &b.0[..]));
            if let Some(positions) = reader.read_positions() {
                for p in positions {
                    min_y = min_y.min(p[1]);
                    max_y = max_y.max(p[1]);
                }
            }
        }
    }

    if min_y.is_finite() && max_y.is_finite() {
        Some((max_y - min_y).abs() * 100.0)
    } else {
        None
    }
}

fn transform_and_write_glb(input_path: &Path, output_path: &Path, scale_factor: f32) -> Result<()> {
    let bytes = fs::read(input_path)
        .with_context(|| format!("failed to read input file: {}", input_path.display()))?;
    let glb = Glb::from_slice(&bytes).context("input VRM is not a GLB container")?;

    let mut json: Value = serde_json::from_slice(glb.json.as_ref())
        .context("failed to parse glTF JSON chunk from VRM")?;

    rename_bones(&mut json);
    remove_vrm_extensions_and_extras(&mut json);
    remove_unsupported_features(&mut json);
    apply_uniform_scale_to_scene_roots(&mut json, scale_factor);

    let json_bytes =
        serde_json::to_vec(&json).context("failed to serialize transformed glTF JSON")?;

    let transformed = Glb {
        header: glb.header,
        json: Cow::Owned(json_bytes),
        bin: glb.bin,
    };

    let mut out = Vec::new();
    transformed
        .to_writer(&mut out)
        .context("failed to write output GLB")?;

    fs::write(output_path, out)
        .with_context(|| format!("failed to write output: {}", output_path.display()))?;

    Ok(())
}

fn rename_bones(json: &mut Value) {
    let map: HashMap<&str, &str> = BONE_MAP.into_iter().collect();

    if let Some(nodes) = json.get_mut("nodes").and_then(Value::as_array_mut) {
        for node in nodes {
            if let Some(current_name) = node.get("name").and_then(Value::as_str) {
                if let Some(new_name) = map.get(current_name) {
                    node["name"] = Value::String((*new_name).to_string());
                }
            }
        }
    }
}

fn remove_vrm_extensions_and_extras(json: &mut Value) {
    if let Some(top_extensions) = json.get_mut("extensions").and_then(Value::as_object_mut) {
        let keys: Vec<String> = top_extensions
            .keys()
            .filter(|key| key.to_ascii_lowercase().contains("vrm"))
            .cloned()
            .collect();
        for key in keys {
            top_extensions.remove(&key);
        }
    }

    if let Some(extensions_used) = json.get_mut("extensionsUsed").and_then(Value::as_array_mut) {
        extensions_used.retain(|entry| {
            entry
                .as_str()
                .map(|name| !name.to_ascii_lowercase().contains("vrm"))
                .unwrap_or(true)
        });
    }

    if let Some(extensions_required) = json
        .get_mut("extensionsRequired")
        .and_then(Value::as_array_mut)
    {
        extensions_required.retain(|entry| {
            entry
                .as_str()
                .map(|name| !name.to_ascii_lowercase().contains("vrm"))
                .unwrap_or(true)
        });
    }

    remove_key_recursively(json, "extras");
}

fn remove_unsupported_features(json: &mut Value) {
    json.as_object_mut().map(|root| {
        root.remove("animations");
    });

    if let Some(meshes) = json.get_mut("meshes").and_then(Value::as_array_mut) {
        for mesh in meshes {
            if let Some(primitives) = mesh.get_mut("primitives").and_then(Value::as_array_mut) {
                for primitive in primitives {
                    primitive.as_object_mut().map(|obj| {
                        obj.remove("targets");
                    });
                }
            }
        }
    }
}

fn apply_uniform_scale_to_scene_roots(json: &mut Value, scale_factor: f32) {
    let root_node_indices = json
        .get("scenes")
        .and_then(Value::as_array)
        .and_then(|scenes| scenes.first())
        .and_then(|scene| scene.get("nodes"))
        .and_then(Value::as_array)
        .cloned();

    let Some(root_node_indices) = root_node_indices else {
        return;
    };

    if let Some(nodes) = json.get_mut("nodes").and_then(Value::as_array_mut) {
        for node_index in root_node_indices
            .into_iter()
            .filter_map(|index| index.as_u64().map(|n| n as usize))
        {
            if let Some(node) = nodes.get_mut(node_index) {
                let existing = node
                    .get("scale")
                    .and_then(Value::as_array)
                    .and_then(|values| {
                        if values.len() == 3 {
                            Some(Vector3::new(
                                values[0].as_f64().unwrap_or(1.0) as f32,
                                values[1].as_f64().unwrap_or(1.0) as f32,
                                values[2].as_f64().unwrap_or(1.0) as f32,
                            ))
                        } else {
                            None
                        }
                    })
                    .unwrap_or(Vector3::new(1.0, 1.0, 1.0));

                let result = existing * scale_factor;
                node["scale"] = serde_json::json!([result.x, result.y, result.z]);
            }
        }
    }
}

fn remove_key_recursively(value: &mut Value, target_key: &str) {
    match value {
        Value::Object(map) => {
            map.remove(target_key);
            for (_, v) in map.iter_mut() {
                remove_key_recursively(v, target_key);
            }
        }
        Value::Array(array) => {
            for v in array.iter_mut() {
                remove_key_recursively(v, target_key);
            }
        }
        _ => {}
    }
}

fn extract_model_name(json: &Value) -> Option<String> {
    json.pointer("/extensions/VRM/meta/name")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            json.get("asset")
                .and_then(|asset| asset.get("generator"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
}

fn extract_author(json: &Value) -> Option<String> {
    json.pointer("/extensions/VRM/meta/authors/0")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            json.get("asset")
                .and_then(|asset| asset.get("copyright"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
}
