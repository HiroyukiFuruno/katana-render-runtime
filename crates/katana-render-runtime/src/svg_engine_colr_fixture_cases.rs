use super::usvg;
use super::{
    STATIC_FONT, assert_render_roundtrip, assert_svg_semantic, fixture_font_database, fixture_path,
    glyph_svg, parse_fixture_svg, read_and_verify_manifest, unicode_for_glyph,
    verify_manifest_file,
};
use skrifa::{
    FontRef, GlyphId, MetadataProvider,
    color::ColorGlyphFormat,
    instance::{LocationRef, Size},
};
use std::{fs, path::Path, sync::Arc};

#[path = "svg_engine_colr_radial_tests.rs"]
mod radial_tests;
type ClipBoxBounds = (f32, f32, f32, f32);

#[path = "svg_engine_colr_fixture_cases_tests.rs"]
mod fixture_cases_tests;

fn verify_static_font_manifest() -> Result<(), String> {
    let manifest = read_and_verify_manifest()?;
    verify_manifest_file(&manifest, STATIC_FONT)
}

fn with_static_font<T>(
    path: &Path,
    callback: impl FnOnce(&FontRef<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let font = FontRef::new(&bytes).map_err(|error| error.to_string())?;
    callback(&font)
}

fn clip_box_bounds(font: &FontRef<'_>, glyph_id: u32) -> Option<ClipBoxBounds> {
    font.color_glyphs()
        .get_with_format(GlyphId::new(glyph_id), ColorGlyphFormat::ColrV1)
        .and_then(|glyph| glyph.bounding_box(LocationRef::default(), Size::unscaled()))
        .map(|bounds| (bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max))
}

fn verify_standard_case(case: &str, glyph_id: u32, semantic: &str) -> Result<(), String> {
    let (tree, database) = fixture_tree(glyph_id, case)?;
    let serialized = tree.to_string(&usvg::WriteOptions::default());
    assert_svg_semantic(&serialized, semantic, false, case)?;
    let reparsed = parse_fixture_svg(&serialized, database, case)?;
    let again = reparsed.to_string(&usvg::WriteOptions::default());
    assert_svg_semantic(&again, semantic, false, case)?;
    assert_render_roundtrip(&tree, &reparsed, case)
}

fn verify_unclipped_glyph(glyph_id: u32) -> Result<(), String> {
    let case = format!("glyph-{glyph_id}-without-clip-box");
    let (tree, database) = fixture_tree(glyph_id, &case)?;
    let serialized = tree.to_string(&usvg::WriteOptions::default());
    if !serialized.contains("<path") {
        return Err(format!("{case} did not produce vector paths"));
    }
    let reparsed = parse_fixture_svg(&serialized, database, &case)?;
    assert_render_roundtrip(&tree, &reparsed, &case)
}

fn fixture_tree(
    glyph_id: u32,
    case: &str,
) -> Result<(usvg::Tree, Arc<usvg::fontdb::Database>), String> {
    fixture_tree_at(&fixture_path(STATIC_FONT), glyph_id, case)
}

fn fixture_tree_at(
    path: &Path,
    glyph_id: u32,
    case: &str,
) -> Result<(usvg::Tree, Arc<usvg::fontdb::Database>), String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let character = unicode_for_glyph(&bytes, glyph_id)?;
    let database = fixture_font_database(path)?;
    let svg = glyph_svg(&database, character, case)?;
    let tree = parse_fixture_svg(&svg, database.clone(), case)?;
    Ok((tree, database))
}
