use serde_json::Value;
use skia_safe::Size;
use std::collections::BTreeSet;

const VIEW_BOX_COMPONENTS: usize = 4;

pub(super) fn svg_size(svg: &str) -> Result<(i32, i32, Size, Value), String> {
    let (width_value, height_value, view_box) = intrinsic_svg_size(svg)?;
    let width = i32::try_from(width_value.ceil() as u64).map_err(|_| "SVG width is too large")?;
    let height =
        i32::try_from(height_value.ceil() as u64).map_err(|_| "SVG height is too large")?;
    let container = Size::new(width_value as f32, height_value as f32);
    Ok((
        width,
        height,
        container,
        serde_json::json!({
            "width": width_value,
            "height": height_value,
            "viewBox": view_box,
            "surfaceRounding": "ceil",
            "localTransform": {"scaleX": 1.0_f32, "scaleY": 1.0_f32, "translateX": 0.0_f32, "translateY": 0.0_f32},
        }),
    ))
}

fn intrinsic_svg_size(svg: &str) -> Result<(f64, f64, Value), String> {
    let document =
        resvg::usvg::roxmltree::Document::parse(svg).map_err(|error| error.to_string())?;
    let root = document.root_element();
    let width = intrinsic_dimension(root, "width")?;
    let height = intrinsic_dimension(root, "height")?;
    let view_box = root
        .attribute("viewBox")
        .map(parse_view_box)
        .transpose()?
        .map_or(Value::Null, |values| serde_json::json!(values));
    Ok((width, height, view_box))
}

fn intrinsic_dimension(
    root: resvg::usvg::roxmltree::Node<'_, '_>,
    name: &str,
) -> Result<f64, String> {
    let value = root
        .attribute(name)
        .ok_or_else(|| format!("diagnostic SVG {name} is absent"))?
        .parse::<f64>()
        .map_err(|error| format!("diagnostic SVG {name} is not unitless: {error}"))?;
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("diagnostic SVG {name} is invalid"));
    }
    Ok(value)
}

fn parse_view_box(value: &str) -> Result<[f64; VIEW_BOX_COMPONENTS], String> {
    let values = value
        .split_whitespace()
        .map(|part| part.parse::<f64>().map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let values: [f64; VIEW_BOX_COMPONENTS] = values
        .try_into()
        .map_err(|_| "diagnostic SVG viewBox must have four values".to_owned())?;
    if values.iter().all(|value| value.is_finite()) {
        Ok(values)
    } else {
        Err("diagnostic SVG viewBox is invalid".to_owned())
    }
}

pub(super) struct VerifiedFace {
    pub(super) path: String,
    pub(super) index: u32,
    pub(super) bytes: Vec<u8>,
    pub(super) bytes_sha256: String,
    pub(super) post_script_name: String,
    pub(super) family: String,
    pub(super) weight: i32,
}

struct FaceMetadata {
    path: String,
    index: u32,
    bytes_sha256: String,
    post_script_name: String,
    family: String,
    weight: i32,
}

pub(super) fn verified_faces(root: &Value) -> Result<Vec<VerifiedFace>, String> {
    let records = root["faces"]
        .as_array()
        .ok_or("font face records are absent")?;
    let mut seen = BTreeSet::new();
    let mut faces = Vec::new();
    for record in records {
        if let Some(face) = verified_face(record)? {
            let key = format!("{}:{}:{}", face.path, face.index, face.bytes_sha256);
            if seen.insert(key) {
                faces.push(face);
            }
        }
    }
    if faces.is_empty() {
        return Err("diagnostic has no verified faces".to_owned());
    }
    Ok(faces)
}

fn verified_face(record: &Value) -> Result<Option<VerifiedFace>, String> {
    if record["identity"] != "physical_verified" {
        return Err("diagnostic face is not physically verified".to_owned());
    }
    let metadata = face_metadata(
        record
            .get("source")
            .ok_or("diagnostic face source is absent")?,
    )?;
    let bytes = read_verified_bytes(&metadata.path, &metadata.bytes_sha256)?;
    Ok(Some(VerifiedFace {
        path: metadata.path,
        index: metadata.index,
        bytes,
        bytes_sha256: metadata.bytes_sha256,
        post_script_name: metadata.post_script_name,
        family: metadata.family,
        weight: metadata.weight,
    }))
}

fn face_metadata(source: &Value) -> Result<FaceMetadata, String> {
    let (path, index, bytes_sha256, post_script_name) = face_location(source)?;
    let (family, weight) = face_style(source)?;
    Ok(FaceMetadata {
        path,
        index,
        bytes_sha256,
        post_script_name,
        family,
        weight,
    })
}

fn face_location(source: &Value) -> Result<(String, u32, String, String), String> {
    let path = source["path"]
        .as_str()
        .ok_or("diagnostic face path is absent")?;
    let index = u32::try_from(
        source["index"]
            .as_u64()
            .ok_or("diagnostic face index is absent")?,
    )
    .map_err(|_| "diagnostic face index is too large")?;
    let bytes_sha256 = source["bytes_sha256"]
        .as_str()
        .ok_or("diagnostic face SHA is absent")?
        .to_owned();
    let post_script_name = source["post_script_name"]
        .as_str()
        .ok_or("diagnostic face PostScript name is absent")?
        .to_owned();
    Ok((path.to_owned(), index, bytes_sha256, post_script_name))
}

fn face_style(source: &Value) -> Result<(String, i32), String> {
    if source["style"] != "Normal" || source["stretch"] != "Normal" {
        return Err("diagnostic face style or stretch is unsupported".to_owned());
    }
    let family = source["families"]
        .as_array()
        .and_then(|families| families.first())
        .and_then(Value::as_str)
        .ok_or("diagnostic face family is absent")?
        .to_owned();
    let weight = i32::try_from(
        source["weight"]
            .as_u64()
            .ok_or("diagnostic face weight is absent")?,
    )
    .map_err(|_| "diagnostic face weight is too large")?;
    Ok((family, weight))
}

fn read_verified_bytes(path: &str, expected_sha: &str) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if super::super::super::super::output::sha256(&bytes) != expected_sha {
        return Err("diagnostic face bytes changed".to_owned());
    }
    Ok(bytes)
}
