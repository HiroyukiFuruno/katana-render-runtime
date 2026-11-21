use super::{COLR_VARIABLE, FONT_SIZE, colr_linear_character, fixture_tree, target_span};
use crate::markdown::svg_rasterize::{
    html_glyph::HtmlGlyphRasterizer, html_glyph_variations,
    physical_font_face::verification_support::font_with_tables,
};
use resvg::usvg::FontVariation;
use std::{fs, path::PathBuf};
use write_fonts::types::Tag;

const VARIABLE_FONT: &str =
    "../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/RobotoFlex.subset.ttf";

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

#[test]
fn variable_instance_rejects_invalid_size_before_caching_a_typeface() -> Result<(), String> {
    let tree = fixture_tree(VARIABLE_FONT, Some("'wght' 700"), 'A')?;
    let (_, span, id) = target_span(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    assert!(rasterizer.face_file(id)?.is_some());
    let error = rasterizer.variable_instance(span, id, 0.0).err();
    assert_eq!(
        error,
        Some(format!("invalid variation font size for face {id:?}"))
    );
    assert!(rasterizer.variable_faces.borrow().instances.is_empty());
    Ok(())
}

#[test]
fn skia_coordinate_clamping_rejects_the_requested_instance_key() -> Result<(), String> {
    const OUT_OF_RANGE_WEIGHT: f32 = 1500.0;
    let tree = fixture_tree(VARIABLE_FONT, Some("'wght' 700"), 'A')?;
    let (_, span, id) = target_span(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    assert!(rasterizer.variable_instance(span, id, FONT_SIZE)?.is_some());
    let file = rasterizer.face_file(id)?.ok_or("variable file missing")?;
    let mut design = fixture_design(&tree, span, id)?;
    let weight = design
        .iter_mut()
        .find(|axis| axis.tag == *b"wght")
        .ok_or("weight axis missing")?;
    *weight = FontVariation::new(*b"wght", OUT_OF_RANGE_WEIGHT);
    let key = super::super::InstanceKey::new(id, &design)?;
    let error = rasterizer
        .create_variable_instance(id, &design, key, file)
        .err();
    assert_eq!(
        error,
        Some(format!(
            "Skia applied variation coordinates differ for face {id:?}"
        ))
    );
    assert_eq!(rasterizer.variable_faces.borrow().instances.len(), 1);
    Ok(())
}

fn fixture_design(
    tree: &resvg::usvg::Tree,
    span: &resvg::usvg::layout::Span,
    id: resvg::usvg::fontdb::ID,
) -> Result<Vec<FontVariation>, String> {
    let (_, design) = html_glyph_variations::resolve_design(
        tree.fontdb(),
        id,
        &span.variations,
        span.font_optical_sizing,
        FONT_SIZE,
    )?;
    Ok(design)
}

#[test]
fn removed_font_cannot_publish_an_already_instantiated_typeface()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("RobotoFlex.ttf");
    let tree = copied_variable_tree(&path)?;
    assert_removed_font_is_rejected(&tree, &path)
}

fn copied_variable_tree(
    path: &std::path::Path,
) -> Result<resvg::usvg::Tree, Box<dyn std::error::Error>> {
    fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(VARIABLE_FONT),
        path,
    )?;
    Ok(fixture_tree(
        path.to_str().ok_or("invalid fixture path")?,
        Some("'wght' 700"),
        'A',
    )?)
}

fn assert_removed_font_is_rejected(
    tree: &resvg::usvg::Tree,
    path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_, span, id) = target_span(tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let instance = rasterizer
        .variable_instance(span, id, FONT_SIZE)?
        .ok_or("instance missing")?;
    let fresh_cache = HtmlGlyphRasterizer::new(tree.fontdb());
    let key = super::super::InstanceKey::new(id, &instance.applied_axes)?;
    fs::remove_file(path)?;
    let error = fresh_cache
        .cache_variable_instance(
            id,
            instance.face.typeface.as_skia().clone(),
            instance.applied_axes.clone(),
            key,
        )
        .err();
    assert_eq!(
        error,
        Some(format!("SVG font face {id:?} cannot be parsed"))
    );
    assert!(fresh_cache.variable_faces.borrow().instances.is_empty());
    Ok(())
}
