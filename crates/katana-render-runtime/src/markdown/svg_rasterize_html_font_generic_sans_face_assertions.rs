use super::super::super::super::super::rasterizer_options_with_font_db;
use super::super::super::super::proof::CreationProof;
use super::super::super::super::{HtmlFontRequest, sources};
use super::super::super::admit;
use super::super::TEXT;
use crate::markdown::svg_rasterize::{RasterizedSvg, SvgRasterizeOps, text_shaping};
use resvg::usvg::fontdb::{Database, FaceInfo, ID, Source};
use std::{path::Path, sync::Arc};

const FONT_SIZE_PX: f32 = 16.0;

pub(in super::super) fn known_database() -> (Database, CreationProof) {
    sources::build_known_html_font_db(&HtmlFontRequest::from_text("sans-serif", TEXT))
}

pub(in super::super) fn admitted(
    database: &Arc<Database>,
    proof: &CreationProof,
    text: &str,
    weight: u16,
    italic: bool,
) -> Option<ID> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        admit(database, proof, "sans-serif", text, weight, italic)
    })
}

pub(in super::super) fn assert_same_face(
    left_db: &Database,
    left_id: ID,
    right_db: &Database,
    right_id: ID,
) -> Result<(), String> {
    let left = left_db.face(left_id).ok_or("known face missing")?;
    let right = right_db.face(right_id).ok_or("eager face missing")?;
    assert_same_file_backed_source(left, right)?;
    assert_same_face_bytes(left_db, left_id, right_db, right_id)?;
    assert_same_face_metadata(left, right);
    Ok(())
}

fn assert_same_file_backed_source(left: &FaceInfo, right: &FaceInfo) -> Result<(), String> {
    let left_path = file_backed_path(&left.source).ok_or("known face is not file-backed")?;
    let right_path = file_backed_path(&right.source).ok_or("eager face is not file-backed")?;
    assert_eq!(left_path, right_path);
    Ok(())
}

fn file_backed_path(source: &Source) -> Option<&Path> {
    match source {
        Source::File(path) | Source::SharedFile(path, _) => Some(path),
        Source::Binary(_) => None,
    }
}

fn assert_same_face_bytes(
    left: &Database,
    left_id: ID,
    right: &Database,
    right_id: ID,
) -> Result<(), String> {
    let bytes_match = left
        .with_face_data(left_id, |left_bytes, left_index| {
            right.with_face_data(right_id, |right_bytes, right_index| {
                left_index == right_index && left_bytes == right_bytes
            })
        })
        .flatten()
        .ok_or("real face bytes are unavailable")?;
    if !bytes_match {
        return Err("real face bytes or source indices differ".into());
    }
    Ok(())
}

fn assert_same_face_metadata(left: &FaceInfo, right: &FaceInfo) {
    assert_eq!(
        (
            left.index,
            &left.families,
            &left.post_script_name,
            left.weight,
            left.style,
            left.stretch,
            left.monospaced
        ),
        (
            right.index,
            &right.families,
            &right.post_script_name,
            right.weight,
            right.style,
            right.stretch,
            right.monospaced
        )
    );
}

pub(in super::super) fn metrics(
    database: &Database,
    id: ID,
) -> Result<(u16, i16, i16, i16), String> {
    database
        .with_face_data(id, |data, index| {
            let face = rustybuzz::ttf_parser::Face::parse(data, index).ok()?;
            Some((
                face.units_per_em(),
                face.ascender(),
                face.descender(),
                face.line_gap(),
            ))
        })
        .flatten()
        .ok_or_else(|| "real face metrics missing".to_string())
}

pub(in super::super) fn eager_render(svg: &str) -> Result<RasterizedSvg, String> {
    let database = Arc::new(sources::build_html_font_db(&HtmlFontRequest::from_markup(
        svg,
    )));
    let mut options = rasterizer_options_with_font_db(database);
    text_shaping::install_html_font_resolver(&mut options);
    crate::markdown::svg_rasterize::font::html::render_html_with_options(svg, &options)
}

fn diagnostic_face_identity(database: &Database, id: ID) -> Result<serde_json::Value, String> {
    let face = database.face(id).ok_or("diagnostic face missing")?;
    let source = match &face.source {
        Source::File(path) => path.display().to_string(),
        Source::Binary(_) => "<binary>".to_string(),
        Source::SharedFile(path, _) => path.display().to_string(),
    };
    Ok(serde_json::json!({
        "databaseFaceIdLocalOnly": format!("{id:?}"),
        "source": source,
        "faceIndex": face.index,
        "families": face.families.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        "postScriptName": face.post_script_name,
        "weight": face.weight.0,
        "style": format!("{:?}", face.style),
        "stretch": format!("{:?}", face.stretch),
    }))
}

pub(in super::super) fn diagnostic_face_data(
    database: &Database,
    id: ID,
) -> Option<(bool, (f32, f32))> {
    database
        .with_face_data(id, |data, index| {
            let face = rustybuzz::ttf_parser::Face::parse(data, index).ok()?;
            let lf_supported = face.glyph_index('\n').is_some();
            let scale = FONT_SIZE_PX / face.units_per_em() as f32;
            let leading = (face.line_gap() as f32 * scale).round();
            let before = (leading / 2.0).floor();
            let ascent = (face.ascender() as f32 * scale).round() + before;
            let descent = (-face.descender() as f32 * scale).round() + leading - before;
            Some((lf_supported, (ascent, descent)))
        })
        .flatten()
}

pub(in super::super) fn diagnostic_face_report(
    database: &Database,
    id: ID,
) -> Result<serde_json::Value, String> {
    let identity = diagnostic_face_identity(database, id)?;
    let (lf_supported, (ascent, descent)) =
        diagnostic_face_data(database, id).ok_or("font face could not be parsed")?;
    Ok(serde_json::json!({
        "identity": identity,
        "supportsLineFeed": lf_supported,
        "lineMetricsAt16px": { "ascent": ascent, "descent": descent },
    }))
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_generic_sans_face_assertions_tests.rs"]
mod tests;

#[cfg(test)]
fn font_path(relative: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

#[cfg(test)]
fn single_face_database(source: Source) -> (Database, ID) {
    let mut database = Database::new();
    let ids = database.load_font_source(source);
    assert_eq!(ids.len(), 1, "expected one real font face");
    (database, ids[0])
}

#[cfg(test)]
fn inspect_source(source: Source, bytes: &[u8]) -> Result<serde_json::Value, String> {
    let (database, id) = single_face_database(source);
    let face = database.face(id).ok_or("font face missing")?;
    let matches =
        database.with_face_data(id, |actual, index| actual == bytes && index == face.index);
    assert_eq!(matches, Some(true));
    diagnostic_face_identity(&database, id)
}
