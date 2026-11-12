use super::{COLR_VARIABLE, FONT_SIZE, colr_linear_character, fixture_tree, target_span};
use crate::markdown::svg_rasterize::{
    html_glyph::HtmlGlyphRasterizer, html_glyph_variations,
    physical_font_face::verification_support::font_with_tables,
};
use std::{fs, path::PathBuf};
use write_fonts::types::Tag;

#[test]
fn readable_axes_without_a_renderable_base_do_not_create_a_variable_instance() -> Result<(), String>
{
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(COLR_VARIABLE);
    let source = fs::read(source).map_err(|error| error.to_string())?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("variable.ttf");
    fs::write(&path, &source).map_err(|error| error.to_string())?;
    let fixture = path.to_str().ok_or("fixture path is not UTF-8")?;
    let tree = fixture_tree(fixture, Some("'SWPS' 10"), colr_linear_character()?)?;
    let (_, span, id) = target_span(&tree)?;
    let incomplete = font_with_tables(&source, &[Tag::new(b"head"), Tag::new(b"fvar")])?;
    fs::write(&path, incomplete).map_err(|error| error.to_string())?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let (_, design) = html_glyph_variations::resolve_design(
        tree.fontdb(),
        id,
        &span.variations,
        span.font_optical_sizing,
        FONT_SIZE,
    )?;
    assert!(!design.is_empty());
    assert!(!rasterizer.face_metadata_at(id, Some(&design))?.unsupported);
    assert!(rasterizer.variable_instance(span, id, FONT_SIZE)?.is_none());
    let cache = rasterizer.variable_faces.borrow();
    assert!(cache.bases.is_empty());
    assert!(cache.instances.is_empty());
    Ok(())
}
