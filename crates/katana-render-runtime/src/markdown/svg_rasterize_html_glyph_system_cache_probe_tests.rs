use super::{HtmlGlyphRasterizer, find_face};
use resvg::usvg::fontdb::{Database, Weight};
use serde_json::{Value, json};
use skia_safe::{FontMgr, Typeface};

const FAMILY: &str = "Trebuchet MS";
const SAMPLE: &str = "AV";

#[test]
fn system_cache_probe_records_physical_binding_for_trebuchet_styles() -> Result<(), String> {
    let database = super::super::super::font::html_font_db_for_text(FAMILY, SAMPLE);
    let rasterizer = HtmlGlyphRasterizer::new(&database);
    let manager = FontMgr::default();
    for weight in [Weight::NORMAL, Weight::BOLD] {
        record_style(&database, &rasterizer, &manager, weight)?;
    }
    Ok(())
}

fn record_style(
    database: &Database,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    manager: &FontMgr,
    weight: Weight,
) -> Result<(), String> {
    eprintln!("{}", query_style(database, rasterizer, manager, weight)?);
    Ok(())
}

fn query_style(
    database: &Database,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    manager: &FontMgr,
    weight: Weight,
) -> Result<Value, String> {
    let id = find_face(database, FAMILY, weight)?;
    let cached = rasterizer
        .ensure_face(id)?
        .ok_or("selected file typeface unavailable")?;
    let file = cached.typeface.as_skia();
    let selected = selected_source(database, rasterizer, id)?;
    let (system, repeated) = matched_typefaces(manager, file)?;
    let existing = source_observation(&system, &selected, false);
    let copied = source_observation(&system, &selected, true);
    Ok(json!({
        "probe": "system-cache",
        "family": FAMILY,
        "weight": weight.0,
        "file_id": file.unique_id(),
        "system_id": system.unique_id(),
        "repeat_id": repeated.unique_id(),
        "repeat_same_id": system.unique_id() == repeated.unique_id(),
        "existing": existing,
        "copied": copied,
    }))
}

fn matched_typefaces(manager: &FontMgr, file: &Typeface) -> Result<(Typeface, Typeface), String> {
    let style = file.font_style();
    let system = manager
        .match_family_style(FAMILY, style)
        .ok_or("system family/style typeface unavailable")?;
    let repeated = manager
        .match_family_style(FAMILY, style)
        .ok_or("repeated system family/style typeface unavailable")?;
    Ok((system, repeated))
}

fn selected_source(
    database: &Database,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: resvg::usvg::fontdb::ID,
) -> Result<(Vec<u8>, u32), String> {
    let (_, index) = rasterizer
        .face_file(id)?
        .ok_or("selected fontdb face is not file-backed")?;
    let bytes = database
        .with_face_data(id, |data, _| data.to_vec())
        .ok_or("selected fontdb face data unavailable")?;
    Ok((bytes, index))
}

fn source_observation(typeface: &Typeface, selected: &(Vec<u8>, u32), copied: bool) -> Value {
    let observed = if copied {
        typeface.to_font_bytes()
    } else {
        typeface
            .to_existing_font_data()
            .map(|(data, index)| (data.as_bytes().to_vec(), index))
    };
    let Some((bytes, index)) = observed else {
        return json!({
            "available": false,
            "bytes_equal": false,
            "index_equal": false,
            "verified": false,
            "mode": if copied { "to_font_bytes" } else { "to_existing_font_data" },
        });
    };
    let bytes_equal = bytes == selected.0;
    let index_equal = index == selected.1;
    json!({
        "available": true,
        "bytes_equal": bytes_equal,
        "index_equal": index_equal,
        "verified": bytes_equal && index_equal,
        "mode": if copied { "to_font_bytes" } else { "to_existing_font_data" },
    })
}
