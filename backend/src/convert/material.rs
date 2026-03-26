use anyhow::{Context, Result};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use serde_json::Value;
use std::io::Cursor;

/// Normalize all materials for SecondLife compatibility.
pub(super) fn normalize_materials_for_secondlife(
    json: &mut Value,
    pbr_enabled: bool,
) -> Result<()> {
    {
        let Some(materials) = json.get_mut("materials").and_then(Value::as_array_mut) else {
            return Ok(());
        };

        for (material_index, material) in materials.iter_mut().enumerate() {
            normalize_single_material(material, material_index, pbr_enabled);
        }
    }

    // Keep extensionsUsed in sync with actual material usage.
    prune_extensions_used(json);

    Ok(())
}

/// Normalize a single material for SecondLife compatibility.
fn normalize_single_material(material: &mut Value, _material_index: usize, pbr_enabled: bool) {
    // Preserve alpha modes; only sanitize alphaCutoff.
    let current_alpha_mode = material
        .get("alphaMode")
        .and_then(Value::as_str)
        .unwrap_or("OPAQUE");

    match current_alpha_mode {
        "MASK" => {
            if let Some(cutoff) = material.get_mut("alphaCutoff") {
                if let Some(val) = cutoff.as_f64() {
                    *cutoff = Value::from(val.clamp(0.0, 1.0));
                }
            } else {
                material["alphaCutoff"] = Value::from(0.5);
            }
        }
        _ => {
            if let Some(obj) = material.as_object_mut() {
                obj.remove("alphaCutoff");
            }
        }
    }

    if pbr_enabled {
        if let Some(pbr) = material.get_mut("pbrMetallicRoughness") {
            if let Some(pbr_obj) = pbr.as_object_mut() {
                if let Some(v) = pbr_obj.get("metallicFactor").and_then(Value::as_f64) {
                    pbr_obj.insert("metallicFactor".to_string(), Value::from(v.clamp(0.0, 1.0)));
                }
                if let Some(v) = pbr_obj.get("roughnessFactor").and_then(Value::as_f64) {
                    pbr_obj.insert(
                        "roughnessFactor".to_string(),
                        Value::from(v.clamp(0.0, 1.0)),
                    );
                }
            }
        }
    } else {
        // Keep baseColorTexture/baseColorFactor for SL texture recognition.
        if let Some(pbr) = material
            .get_mut("pbrMetallicRoughness")
            .and_then(Value::as_object_mut)
        {
            pbr.remove("metallicFactor");
            pbr.remove("roughnessFactor");
            pbr.remove("metallicRoughnessTexture");
        }
    }

    // SL PBR supports normalTexture, emissiveTexture, occlusionTexture and
    // metallicRoughnessTexture (ORM — R=occlusion, G=roughness, B=metallic).
    // Keep all of these for visual quality in SL.
    // The extensions sub-keys inside texture-info objects are stripped below
    // to prevent SL's upload dialog from failing to parse the texture index.
    let material_name = material
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();

    let is_eye_like = material_name.contains("eye")
        || material_name.contains("eyelash")
        || material_name.contains("brow");

    if is_eye_like {
        material["doubleSided"] = Value::Bool(true);
    }

    // Remove ALL material extensions.
    //
    // SL's mesh upload dialog uses a separate (older) code path from the
    // runtime glTF renderer.  When material.extensions contains ANY entry
    // — including the otherwise-supported KHR_materials_unlit — the upload
    // dialog fails to locate texture references and reports 0 textures.
    if let Some(obj) = material.as_object_mut() {
        obj.remove("extensions");
    }

    // ─── Strip extensions from texture-info objects ───────────────────────────
    // SL's mesh importer cannot parse a texture-info object that contains an
    // `extensions` sub-key (e.g. KHR_texture_transform).  When it encounters
    // one it fails to extract the `index` field and reports 0 textures.
    // KHR_texture_transform with identity offset/scale is a no-op anyway.
    strip_texture_info_extensions(material);
}

/// Remove `extensions` from every texture-info object in a material.
/// Texture-info objects are those that contain an `index` integer (e.g.
/// baseColorTexture, normalTexture, emissiveTexture, etc.).
fn strip_texture_info_extensions(material: &mut Value) {
    // Helper: if `val` looks like a texture-info object, strip extensions.
    fn clean_tex_info(val: &mut Value) {
        if val.get("index").and_then(Value::as_u64).is_some() {
            if let Some(obj) = val.as_object_mut() {
                obj.remove("extensions");
            }
        }
    }

    // pbrMetallicRoughness sub-textures
    if let Some(pbr) = material.get_mut("pbrMetallicRoughness") {
        for key in &["baseColorTexture", "metallicRoughnessTexture"] {
            if let Some(tex) = pbr.get_mut(*key) {
                clean_tex_info(tex);
            }
        }
    }

    // Top-level texture slots
    for key in &["normalTexture", "occlusionTexture", "emissiveTexture"] {
        if let Some(tex) = material.get_mut(*key) {
            clean_tex_info(tex);
        }
    }
}

/// Rebuild extensionsUsed from actual remaining usage.
fn prune_extensions_used(json: &mut Value) {
    let used: std::collections::HashSet<String> = {
        let mut set = std::collections::HashSet::new();

        if let Some(materials) = json.get("materials").and_then(Value::as_array) {
            for mat in materials {
                if let Some(exts) = mat.get("extensions").and_then(Value::as_object) {
                    for k in exts.keys() {
                        set.insert(k.clone());
                    }
                }

                for tex_key in &[
                    "baseColorTexture",
                    "metallicRoughnessTexture",
                    "normalTexture",
                    "occlusionTexture",
                    "emissiveTexture",
                ] {
                    let tex_ref = mat
                        .get("pbrMetallicRoughness")
                        .and_then(|p| p.get(*tex_key))
                        .or_else(|| mat.get(*tex_key));

                    if let Some(t) = tex_ref {
                        if let Some(exts) = t.get("extensions").and_then(Value::as_object) {
                            for k in exts.keys() {
                                set.insert(k.clone());
                            }
                        }
                    }
                }
            }
        }

        set
    };

    if let Some(ext_used) = json.get_mut("extensionsUsed").and_then(Value::as_array_mut) {
        ext_used.retain(|entry| {
            entry
                .as_str()
                .map(|name| used.contains(name))
                .unwrap_or(false)
        });
    }
}

/// Validate texture buffer integrity and fix issues.
pub(super) fn validate_and_fix_texture_references(json: &mut Value) -> Result<()> {
    let image_count = json
        .get("images")
        .and_then(Value::as_array)
        .map(|arr| arr.len())
        .unwrap_or(0);

    if image_count == 0 {
        return Ok(());
    }

    let texture_count = json
        .get("textures")
        .and_then(Value::as_array)
        .map(|arr| arr.len())
        .unwrap_or(0);

    let buffer_view_count = json
        .get("bufferViews")
        .and_then(Value::as_array)
        .map(|arr| arr.len())
        .unwrap_or(0);

    if let Some(materials) = json.get_mut("materials").and_then(Value::as_array_mut) {
        for material in materials {
            validate_material_texture_references(material, texture_count);
        }
    }

    if let Some(images) = json.get_mut("images").and_then(Value::as_array_mut) {
        for image in images {
            if let Some(bv_index) = image.get("bufferView").and_then(Value::as_u64) {
                if bv_index as usize >= buffer_view_count {
                    image["_invalid"] = Value::Bool(true);
                }
            }

            if image.get("mimeType").is_none() {
                image["mimeType"] = Value::String("image/png".to_string());
            }
        }
    }

    Ok(())
}

/// Merge AO + metallic-roughness into a single ORM map and align texture refs.
///
/// If `occlusionTexture` and `pbrMetallicRoughness.metallicRoughnessTexture`
/// point to different textures, this function rewrites the metallic-roughness
/// image so that:
/// - R channel comes from AO texture (if available)
/// - G/B channels come from metallic-roughness texture
/// Then both material slots will reference the same texture index.
pub(super) fn merge_orm_textures_into_single_map(json: &mut Value, bin: &mut [u8]) -> Result<()> {
    let material_count = json
        .get("materials")
        .and_then(Value::as_array)
        .map(|m| m.len())
        .unwrap_or(0);

    for material_index in 0..material_count {
        let (Some(ao_tex), Some(mr_tex)) = (
            texture_index_at(json, material_index, "occlusionTexture"),
            texture_index_at(
                json,
                material_index,
                "pbrMetallicRoughness.metallicRoughnessTexture",
            ),
        ) else {
            continue;
        };

        if ao_tex == mr_tex {
            continue;
        }

        let ao_bytes = get_image_bytes_by_texture_index(json, bin, ao_tex)?;
        let mr_bytes = get_image_bytes_by_texture_index(json, bin, mr_tex)?;

        let merged = match (ao_bytes, mr_bytes) {
            (Some(ao), Some(mr)) => {
                let merged_img = compose_orm_image(&ao, &mr).with_context(|| {
                    format!("failed to compose ORM for material {}", material_index)
                })?;
                Some(encode_png(&merged_img).context("failed to encode merged ORM image")?)
            }
            _ => None,
        };

        if let Some(bytes) = merged {
            replace_image_bytes_by_texture_index(json, bin, mr_tex, &bytes)?;
            set_image_mime_by_texture_index(json, mr_tex, "image/png");
        }

        set_texture_index_at(json, material_index, "occlusionTexture", mr_tex);
    }

    Ok(())
}

fn texture_index_at(json: &Value, material_index: usize, path: &str) -> Option<usize> {
    let mat = json
        .get("materials")
        .and_then(Value::as_array)
        .and_then(|m| m.get(material_index))?;

    let tex_info = match path {
        "occlusionTexture" => mat.get("occlusionTexture"),
        "pbrMetallicRoughness.metallicRoughnessTexture" => mat
            .get("pbrMetallicRoughness")
            .and_then(|p| p.get("metallicRoughnessTexture")),
        _ => None,
    }?;

    tex_info
        .get("index")
        .and_then(Value::as_u64)
        .map(|v| v as usize)
}

fn set_texture_index_at(json: &mut Value, material_index: usize, path: &str, new_index: usize) {
    let Some(material) = json
        .get_mut("materials")
        .and_then(Value::as_array_mut)
        .and_then(|m| m.get_mut(material_index))
    else {
        return;
    };

    let target = match path {
        "occlusionTexture" => material.get_mut("occlusionTexture"),
        "pbrMetallicRoughness.metallicRoughnessTexture" => material
            .get_mut("pbrMetallicRoughness")
            .and_then(|p| p.get_mut("metallicRoughnessTexture")),
        _ => None,
    };

    if let Some(tex_info) = target {
        tex_info["index"] = Value::from(new_index as u64);
    }
}

fn get_image_bytes_by_texture_index(
    json: &Value,
    bin: &[u8],
    texture_index: usize,
) -> Result<Option<Vec<u8>>> {
    let texture = json
        .get("textures")
        .and_then(Value::as_array)
        .and_then(|t| t.get(texture_index));
    let Some(texture) = texture else {
        return Ok(None);
    };

    let Some(image_index) = texture
        .get("source")
        .and_then(Value::as_u64)
        .map(|v| v as usize)
    else {
        return Ok(None);
    };

    let image = json
        .get("images")
        .and_then(Value::as_array)
        .and_then(|i| i.get(image_index));
    let Some(image) = image else {
        return Ok(None);
    };

    let Some(buffer_view_index) = image
        .get("bufferView")
        .and_then(Value::as_u64)
        .map(|v| v as usize)
    else {
        return Ok(None);
    };

    let buffer_view = json
        .get("bufferViews")
        .and_then(Value::as_array)
        .and_then(|v| v.get(buffer_view_index))
        .context("bufferView not found for image")?;

    let offset = buffer_view
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let length = buffer_view
        .get("byteLength")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let end = offset.saturating_add(length);

    if end > bin.len() || length == 0 {
        return Ok(None);
    }

    Ok(Some(bin[offset..end].to_vec()))
}

fn replace_image_bytes_by_texture_index(
    json: &mut Value,
    bin: &mut [u8],
    texture_index: usize,
    new_bytes: &[u8],
) -> Result<()> {
    let Some(image_index) = json
        .get("textures")
        .and_then(Value::as_array)
        .and_then(|t| t.get(texture_index))
        .and_then(|t| t.get("source"))
        .and_then(Value::as_u64)
        .map(|v| v as usize)
    else {
        return Ok(());
    };

    let Some(buffer_view_index) = json
        .get("images")
        .and_then(Value::as_array)
        .and_then(|i| i.get(image_index))
        .and_then(|img| img.get("bufferView"))
        .and_then(Value::as_u64)
        .map(|v| v as usize)
    else {
        return Ok(());
    };

    let Some(buffer_view) = json
        .get_mut("bufferViews")
        .and_then(Value::as_array_mut)
        .and_then(|v| v.get_mut(buffer_view_index))
    else {
        return Ok(());
    };

    let offset = buffer_view
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let length = buffer_view
        .get("byteLength")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let end = offset.saturating_add(length);

    if end > bin.len() || new_bytes.len() > length {
        return Ok(());
    }

    bin[offset..offset + new_bytes.len()].copy_from_slice(new_bytes);
    if new_bytes.len() < length {
        for b in &mut bin[offset + new_bytes.len()..end] {
            *b = 0;
        }
    }

    Ok(())
}

fn set_image_mime_by_texture_index(json: &mut Value, texture_index: usize, mime: &str) {
    let Some(image_index) = json
        .get("textures")
        .and_then(Value::as_array)
        .and_then(|t| t.get(texture_index))
        .and_then(|t| t.get("source"))
        .and_then(Value::as_u64)
        .map(|v| v as usize)
    else {
        return;
    };

    if let Some(image) = json
        .get_mut("images")
        .and_then(Value::as_array_mut)
        .and_then(|i| i.get_mut(image_index))
    {
        image["mimeType"] = Value::String(mime.to_string());
    }
}

fn compose_orm_image(ao_bytes: &[u8], mr_bytes: &[u8]) -> Result<DynamicImage> {
    let ao = image::load_from_memory(ao_bytes).context("failed to decode AO image")?;
    let mr = image::load_from_memory(mr_bytes).context("failed to decode MR image")?;

    let (w, h) = (mr.width(), mr.height());
    let ao_resized = if ao.width() != w || ao.height() != h {
        ao.resize_exact(w, h, image::imageops::FilterType::Triangle)
    } else {
        ao
    };

    let ao_rgba = ao_resized.to_rgba8();
    let mr_rgba = mr.to_rgba8();
    let mut out: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let ao_px = ao_rgba.get_pixel(x, y);
            let mr_px = mr_rgba.get_pixel(x, y);
            out.put_pixel(x, y, Rgba([ao_px[0], mr_px[1], mr_px[2], 255]));
        }
    }

    Ok(DynamicImage::ImageRgba8(out))
}

fn encode_png(image: &DynamicImage) -> Result<Vec<u8>> {
    let mut out = Vec::<u8>::new();
    image
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .context("failed to encode png")?;
    Ok(out)
}

/// Validate material texture references against available textures.
fn validate_material_texture_references(material: &mut Value, texture_count: usize) {
    let mut invalid_refs = Vec::new();

    if let Some(pbr) = material.get("pbrMetallicRoughness") {
        if let Some(idx) = pbr
            .get("baseColorTexture")
            .and_then(|t| t.get("index"))
            .and_then(Value::as_u64)
        {
            if idx as usize >= texture_count {
                invalid_refs.push("baseColorTexture");
            }
        }

        if let Some(idx) = pbr
            .get("metallicRoughnessTexture")
            .and_then(|t| t.get("index"))
            .and_then(Value::as_u64)
        {
            if idx as usize >= texture_count {
                invalid_refs.push("metallicRoughnessTexture");
            }
        }
    }

    for tex_field in &["normalTexture", "occlusionTexture", "emissiveTexture"] {
        if let Some(idx) = material
            .get(*tex_field)
            .and_then(|t| t.get("index"))
            .and_then(Value::as_u64)
        {
            if idx as usize >= texture_count {
                invalid_refs.push(*tex_field);
            }
        }
    }

    if let Some(material_obj) = material.as_object_mut() {
        for invalid_ref in invalid_refs {
            match invalid_ref {
                "baseColorTexture" => {
                    if let Some(pbr) = material_obj.get_mut("pbrMetallicRoughness") {
                        if let Some(p) = pbr.as_object_mut() {
                            p.remove("baseColorTexture");
                        }
                    }
                }
                "metallicRoughnessTexture" => {
                    if let Some(pbr) = material_obj.get_mut("pbrMetallicRoughness") {
                        if let Some(p) = pbr.as_object_mut() {
                            p.remove("metallicRoughnessTexture");
                        }
                    }
                }
                "normalTexture" => {
                    material_obj.remove("normalTexture");
                }
                "occlusionTexture" => {
                    material_obj.remove("occlusionTexture");
                }
                "emissiveTexture" => {
                    material_obj.remove("emissiveTexture");
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn given_mask_mode_eye_material_when_normalizing_then_mask_preserved() {
        let mut material = json!({
            "name": "EyeIris",
            "alphaMode": "MASK",
            "alphaCutoff": 0.5
        });

        normalize_single_material(&mut material, 0, true);

        assert_eq!(material["alphaMode"], "MASK");
        assert!(material.get("alphaCutoff").is_some());
    }

    #[test]
    fn given_mask_mode_material_when_normalizing_then_mask_preserved_and_cutoff_clamped() {
        let mut material = json!({
            "name": "HairBase",
            "alphaMode": "MASK",
            "alphaCutoff": 0.7
        });

        normalize_single_material(&mut material, 0, true);

        assert_eq!(material["alphaMode"], "MASK");
        assert_eq!(material["alphaCutoff"].as_f64().unwrap(), 0.7);
    }

    #[test]
    fn given_pbr_values_out_of_range_when_normalizing_then_clamped() {
        let mut material = json!({
            "name": "TestMaterial",
            "pbrMetallicRoughness": {
                "metallicFactor": 1.5,
                "roughnessFactor": -0.2
            }
        });

        normalize_single_material(&mut material, 0, true);

        let pbr = &material["pbrMetallicRoughness"];
        assert_eq!(pbr["metallicFactor"], 1.0);
        assert_eq!(pbr["roughnessFactor"], 0.0);
    }

    #[test]
    fn given_eye_material_when_normalizing_then_double_sided_enabled() {
        let mut material = json!({
            "name": "mEyeLeft"
        });

        normalize_single_material(&mut material, 0, true);

        assert_eq!(material["doubleSided"], true);
    }

    #[test]
    fn given_pbr_disabled_when_normalizing_then_metallic_fields_removed_but_color_tex_kept() {
        let mut material = json!({
            "name": "Body",
            "pbrMetallicRoughness": {
                "metallicFactor": 0.8,
                "roughnessFactor": 0.2,
                "metallicRoughnessTexture": { "index": 2 },
                "baseColorTexture": { "index": 0 },
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0]
            }
        });

        normalize_single_material(&mut material, 0, false);

        let pbr = material.get("pbrMetallicRoughness").unwrap();
        assert!(pbr.get("metallicFactor").is_none());
        assert!(pbr.get("roughnessFactor").is_none());
        assert!(pbr.get("metallicRoughnessTexture").is_none());
        assert!(pbr.get("baseColorTexture").is_some());
        assert!(pbr.get("baseColorFactor").is_some());
    }

    #[test]
    fn given_vrmc_mtoon_extension_when_normalizing_then_all_extensions_removed() {
        // ALL material.extensions must be removed — including KHR_materials_unlit.
        // SL's upload dialog reports 0 textures when any extension is present,
        // even ones it knows how to render.
        let mut material = json!({
            "name": "Body",
            "extensions": {
                "KHR_materials_unlit": {},
                "VRMC_materials_mtoon": { "specVersion": "1.0" }
            },
            "pbrMetallicRoughness": {
                "baseColorTexture": { "index": 0 }
            }
        });

        normalize_single_material(&mut material, 0, true);

        assert!(
            material.get("extensions").is_none(),
            "all material extensions must be removed"
        );
    }

    #[test]
    fn given_vrm_extensions_when_normalizing_then_extensions_key_removed() {
        let mut material = json!({
            "name": "Hair",
            "extensions": {
                "VRMC_materials_mtoon": { "specVersion": "1.0" }
            }
        });

        normalize_single_material(&mut material, 0, true);

        assert!(material.get("extensions").is_none());
    }

    #[test]
    fn given_khr_texture_transform_in_base_color_texture_when_normalizing_then_stripped() {
        // SL's importer cannot read texture index when extensions sub-key is
        // present inside a texture-info object — it reports 0 textures.
        let mut material = json!({
            "name": "Body",
            "pbrMetallicRoughness": {
                "baseColorTexture": {
                    "index": 0,
                    "texCoord": 0,
                    "extensions": {
                        "KHR_texture_transform": { "offset": [0, 0], "scale": [1, 1] }
                    }
                }
            }
        });

        normalize_single_material(&mut material, 0, true);

        let bct = material["pbrMetallicRoughness"]["baseColorTexture"]
            .as_object()
            .unwrap();
        assert_eq!(bct["index"], 0, "index must be preserved");
        assert!(
            bct.get("extensions").is_none(),
            "extensions must be stripped from texture-info"
        );
    }

    #[test]
    fn given_stale_vrmc_in_extensions_used_when_pruning_then_removed() {
        let mut json = json!({
            "extensionsUsed": [
                "KHR_texture_transform",
                "KHR_materials_unlit",
                "VRMC_materials_mtoon"
            ],
            "materials": [
                {
                    "extensions": {
                        "KHR_materials_unlit": {}
                    },
                    "pbrMetallicRoughness": {
                        "baseColorTexture": {
                            "index": 0,
                            "extensions": {
                                "KHR_texture_transform": { "scale": [1, 1] }
                            }
                        }
                    }
                }
            ]
        });

        prune_extensions_used(&mut json);

        let ext_used = json["extensionsUsed"].as_array().unwrap();
        let names: Vec<&str> = ext_used.iter().filter_map(Value::as_str).collect();
        assert!(names.contains(&"KHR_texture_transform"));
        assert!(names.contains(&"KHR_materials_unlit"));
        assert!(!names.contains(&"VRMC_materials_mtoon"));
    }
}
