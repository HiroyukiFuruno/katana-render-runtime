use super::HtmlGlyphRasterizer;
use crate::markdown::svg_rasterize::font_file_fixture;
use crate::markdown::svg_rasterize::physical_font_face::verification_support::font_with_only;
use resvg::usvg::fontdb::{Database, ID, Source};
use std::path::Path;
use std::sync::Arc;
use write_fonts::types::Tag;

fn shared_font_fixture() -> Result<(tempfile::TempDir, Database, ID), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("font.ttf");
    std::fs::write(&path, font_file_fixture::FONT_BYTES).map_err(|error| error.to_string())?;
    let data = Arc::new(font_file_fixture::FONT_BYTES.to_vec());
    let mut database = Database::new();
    let ids = database.load_font_source(Source::SharedFile(path, data));
    let id = *ids.first().ok_or("shared fixture face missing")?;
    Ok((directory, database, id))
}

#[test]
fn registered_shared_font_missing_on_disk_rejects_face_and_recovers() -> Result<(), String> {
    assert_shared_font_recovers(|path| std::fs::remove_file(path), "cannot be mapped")
}

#[test]
fn registered_shared_font_changed_on_disk_rejects_face_and_recovers() -> Result<(), String> {
    assert_shared_font_recovers(
        |path| {
            let mut changed = font_file_fixture::FONT_BYTES.to_vec();
            changed.push(0);
            std::fs::write(path, changed)
        },
        "differs from parsed face",
    )
}

fn assert_shared_font_recovers(
    change: impl FnOnce(&Path) -> std::io::Result<()>,
    expected: &str,
) -> Result<(), String> {
    let (directory, database, id) = shared_font_fixture()?;
    let path = directory.path().join("font.ttf");
    change(&path).map_err(|error| error.to_string())?;
    let rasterizer = HtmlGlyphRasterizer::new(&database);
    assert!(
        rasterizer
            .ensure_face(id)
            .is_err_and(|error| error.contains(expected))
    );
    std::fs::write(&path, font_file_fixture::FONT_BYTES).map_err(|error| error.to_string())?;
    assert!(rasterizer.ensure_face(id)?.is_some());
    Ok(())
}

#[test]
fn registered_file_font_corruption_rejects_metadata_and_recovers() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = font_file_fixture::create(
        directory.path().join("font.ttf"),
        font_file_fixture::FONT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(&fixture.path, b"invalid font").map_err(|error| error.to_string())?;
    let rasterizer = HtmlGlyphRasterizer::new(&fixture.database);
    assert!(
        rasterizer
            .ensure_face(fixture.face_id)
            .is_err_and(|error| error.contains("cannot be parsed"))
    );
    std::fs::write(&fixture.path, font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())?;
    assert!(rasterizer.ensure_face(fixture.face_id)?.is_some());
    Ok(())
}

#[test]
fn file_font_with_readable_metadata_but_no_typeface_is_not_cached_as_usable() -> Result<(), String>
{
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = font_file_fixture::create(
        directory.path().join("font.ttf"),
        font_file_fixture::FONT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    let header = font_with_only(Tag::new(b"head"))?;
    std::fs::write(&fixture.path, &header).map_err(|error| error.to_string())?;
    let rasterizer = HtmlGlyphRasterizer::new(&fixture.database);
    let metadata = rasterizer.face_metadata(fixture.face_id)?;
    assert!(metadata.upem > 0.0 && !metadata.unsupported);
    assert!(rasterizer.ensure_face(fixture.face_id)?.is_none());
    std::fs::write(&fixture.path, font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())?;
    assert!(rasterizer.ensure_face(fixture.face_id)?.is_none());
    assert!(
        HtmlGlyphRasterizer::new(&fixture.database)
            .ensure_face(fixture.face_id)?
            .is_some()
    );
    Ok(())
}

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
