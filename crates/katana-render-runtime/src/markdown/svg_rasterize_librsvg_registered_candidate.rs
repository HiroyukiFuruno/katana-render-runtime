use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use cairo::FontType;
use pango::prelude::*;
use pangocairo::FontMap;
use resvg::usvg;
use rustybuzz::ttf_parser::Face;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::{RasterizedSvg, SvgRasterizeError};

struct DefaultMapGuard {
    previous: FontMap,
}

impl Drop for DefaultMapGuard {
    fn drop(&mut self) {
        FontMap::set_default(Some(&self.previous));
    }
}

pub(super) fn render(
    svg: &str,
    tree: &usvg::Tree,
    scale: f32,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    let temp = TempDir::new().map_err(|error| failure(format!("font temp dir: {error}")))?;
    let map = make_font_map()?;
    let previous = FontMap::default()
        .dynamic_cast::<FontMap>()
        .map_err(|_| failure("current default map is not a Cairo font map"))?;
    let guard = DefaultMapGuard { previous };
    FontMap::set_default(Some(&map));
    register_used_faces(tree, &map, temp.path())?;
    let result = super::render_outlined_with_skia(svg, tree.size(), scale);
    drop(guard);
    result
}

fn make_font_map() -> Result<FontMap, SvgRasterizeError> {
    FontMap::for_font_type(FontType::FontTypeFt)
        .ok_or_else(|| failure("Pango FT font map is unavailable"))?
        .dynamic_cast::<FontMap>()
        .map_err(|_| failure("Pango FT map is not a Cairo font map"))
}

fn register_used_faces(
    tree: &usvg::Tree,
    map: &FontMap,
    directory: &Path,
) -> Result<(), SvgRasterizeError> {
    let (ids, _) = super::super::skia_registered_candidate::used_face_ids(tree)?;
    for id in ids {
        register_face(tree.fontdb(), id, map, directory)?;
    }
    Ok(())
}

fn register_face(
    database: &usvg::fontdb::Database,
    id: usvg::fontdb::ID,
    map: &FontMap,
    directory: &Path,
) -> Result<(), SvgRasterizeError> {
    let face = database
        .face(id)
        .ok_or_else(|| failure(format!("font face {id:?} missing")))?;
    let (bytes, index) = database
        .with_face_data(id, |bytes, index| (bytes.to_vec(), index))
        .ok_or_else(|| failure(format!("font face {id:?} data missing")))?;
    let registered = static_face_bytes(bytes, index, id)?;
    let path = directory.join(format!("face-{:?}.ttf", id));
    write_new(&path, &registered)?;
    map.add_font_file(&path)
        .map_err(|error| failure(format!("Pango add_font_file failed: {error}")))?;
    verify_loaded_font(map, face, &registered, id)
}

fn static_face_bytes(
    bytes: Vec<u8>,
    index: u32,
    id: usvg::fontdb::ID,
) -> Result<Vec<u8>, SvgRasterizeError> {
    if !Face::parse(&bytes, index)
        .map_err(|_| failure(format!("font face {id:?} parse failed")))?
        .variation_axes()
        .is_empty()
    {
        return Err(failure(format!("variable font face {id:?} is unsupported")));
    }
    if index == 0 && rustybuzz::ttf_parser::fonts_in_collection(&bytes).is_none() {
        Ok(bytes)
    } else {
        super::super::physical_font_face::repack_face(&bytes, index)
            .map_err(|message| failure(format!("font face {id:?} repack failed: {message}")))
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), SvgRasterizeError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| failure(format!("font file create failed: {error}")))?;
    file.write_all(bytes)
        .map_err(|error| failure(format!("font file write failed: {error}")))
}

fn verify_loaded_font(
    map: &FontMap,
    face: &usvg::fontdb::FaceInfo,
    bytes: &[u8],
    id: usvg::fontdb::ID,
) -> Result<(), SvgRasterizeError> {
    let family = face
        .families
        .first()
        .map(|(name, _)| name.as_str())
        .ok_or_else(|| failure(format!("font face {id:?} has no family")))?;
    let context = map.create_context();
    let description = description_for_face(family, face);
    let font = context
        .load_font(&description)
        .ok_or_else(|| failure(format!("Pango could not load font face {id:?}")))?;
    verify_checksum(&font, bytes, id)
}

fn description_for_face(family: &str, face: &usvg::fontdb::FaceInfo) -> pango::FontDescription {
    let mut description = pango::FontDescription::new();
    description.set_family(family);
    description.set_weight(pango::Weight::__Unknown(i32::from(face.weight.0)));
    description.set_style(match face.style {
        usvg::fontdb::Style::Italic => pango::Style::Italic,
        usvg::fontdb::Style::Oblique => pango::Style::Oblique,
        usvg::fontdb::Style::Normal => pango::Style::Normal,
    });
    description.set_stretch(match face.stretch {
        usvg::fontdb::Stretch::UltraCondensed => pango::Stretch::UltraCondensed,
        usvg::fontdb::Stretch::ExtraCondensed => pango::Stretch::ExtraCondensed,
        usvg::fontdb::Stretch::Condensed => pango::Stretch::Condensed,
        usvg::fontdb::Stretch::SemiCondensed => pango::Stretch::SemiCondensed,
        usvg::fontdb::Stretch::Normal => pango::Stretch::Normal,
        usvg::fontdb::Stretch::SemiExpanded => pango::Stretch::SemiExpanded,
        usvg::fontdb::Stretch::Expanded => pango::Stretch::Expanded,
        usvg::fontdb::Stretch::ExtraExpanded => pango::Stretch::ExtraExpanded,
        usvg::fontdb::Stretch::UltraExpanded => pango::Stretch::UltraExpanded,
    });
    description
}

fn verify_checksum(
    font: &pango::Font,
    bytes: &[u8],
    id: usvg::fontdb::ID,
) -> Result<(), SvgRasterizeError> {
    let value: Value = serde_json::from_slice(font.serialize().as_ref())
        .map_err(|error| failure(format!("Pango font serialization failed: {error}")))?;
    let expected = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let actual = value
        .get("checksum")
        .and_then(Value::as_str)
        .ok_or_else(|| failure(format!("Pango font checksum missing for {id:?}")))?;
    if actual != expected {
        return Err(failure(format!(
            "Pango font checksum mismatch for {id:?}: loaded={}, expected={expected}, actual={actual}",
            font.describe()
        )));
    }
    Ok(())
}

fn failure(message: impl Into<String>) -> SvgRasterizeError {
    SvgRasterizeError::RasterizeFailed(message.into())
}

#[cfg(test)]
#[path = "svg_rasterize_librsvg_registered_candidate_tests.rs"]
mod tests;
