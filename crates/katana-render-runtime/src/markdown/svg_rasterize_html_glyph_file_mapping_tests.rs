use super::HtmlGlyphRasterizer;
use crate::markdown::svg_rasterize::font_file_fixture;
use resvg::usvg::fontdb::{Database, ID};
use std::path::Path;

#[test]
fn map_face_data_requires_matching_available_file_and_registered_face() -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("krr-font-pin-{}.ttf", std::process::id()));
    let fixture = font_file_fixture::create(path, font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())?;
    let rasterizer = HtmlGlyphRasterizer::new(&fixture.database);
    assert!(registered_file_matches(
        &rasterizer,
        fixture.face_id,
        &fixture.path
    ));
    assert_different_file_rejected(&rasterizer, fixture.face_id)?;
    assert_missing_file_rejected(&rasterizer, fixture.face_id)?;
    let empty_database = Database::new();
    let empty_rasterizer = HtmlGlyphRasterizer::new(&empty_database);
    assert_map_error(
        &empty_rasterizer,
        fixture.face_id,
        &fixture.path,
        "data is unavailable",
    )
}

fn registered_file_matches(rasterizer: &HtmlGlyphRasterizer<'_>, id: ID, path: &Path) -> bool {
    rasterizer.map_face_data(id, path).is_ok()
}

fn assert_different_file_rejected(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
) -> Result<(), String> {
    let mismatch_path =
        std::env::temp_dir().join(format!("krr-font-mismatch-{}.ttf", std::process::id()));
    let mut mismatch = font_file_fixture::FONT_BYTES.to_vec();
    mismatch.push(0);
    std::fs::write(&mismatch_path, mismatch).map_err(|error| error.to_string())?;
    let result = assert_map_error(rasterizer, id, &mismatch_path, "differs from parsed face");
    std::fs::remove_file(&mismatch_path).map_err(|error| error.to_string())?;
    result
}

fn assert_missing_file_rejected(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
) -> Result<(), String> {
    let missing_path =
        std::env::temp_dir().join(format!("krr-font-missing-{}.ttf", std::process::id()));
    if missing_path.exists() {
        return Err("missing-font fixture path unexpectedly exists".to_owned());
    }
    assert_map_error(rasterizer, id, &missing_path, "cannot be mapped")
}

fn assert_map_error(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
    path: &Path,
    expected: &str,
) -> Result<(), String> {
    let error = rasterizer
        .map_face_data(id, path)
        .err()
        .ok_or_else(|| format!("physical font path unexpectedly accepted: {expected}"))?;
    if !error.contains(expected) {
        return Err(format!(
            "expected font error containing {expected:?}, got {error:?}"
        ));
    }
    Ok(())
}
