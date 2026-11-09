use super::super::{HtmlGlyphRasterizer, html_glyph_variations, usvg};
use resvg::usvg::{
    Options, Tree,
    fontdb::{Database, Source},
};
use skrifa::{FontRef, MetadataProvider};
use std::{path::PathBuf, sync::Arc};

const FONT_SIZE: f32 = 20.0;
const STATIC_FONT: &str = "assets/fonts/NotoSans-Regular.ttf";
const COLR_VARIABLE: &str = "../../tests/fixtures/svg-engine/colr-public-fonts/colr_1_variable.ttf";
const COLR_LINEAR_GLYPH: u32 = 9;

fn fixture_tree(path: &str, variations: Option<&str>, text: char) -> Result<Tree, String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
    let mut database = Database::new();
    let id = *database
        .load_font_source(Source::File(path))
        .first()
        .ok_or("licensed variable fixture has no face")?;
    let family = database
        .face(id)
        .and_then(|face| face.families.first())
        .ok_or("licensed variable fixture has no family")?
        .0
        .clone();
    let settings = match variations {
        Some(value) => format!(r#" font-variation-settings="{value}""#),
        None => String::new(),
    };
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="target" font-family="{family}" font-size="{FONT_SIZE}" font-optical-sizing="none"{settings}>{text}</text></svg>"#
    );
    Tree::from_str(
        &svg,
        &Options {
            fontdb: Arc::new(database),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}

fn colr_linear_character() -> Result<char, String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(COLR_VARIABLE);
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let font = FontRef::new(&bytes).map_err(|error| format!("invalid COLR fixture: {error}"))?;
    font.charmap()
        .mappings()
        .find_map(|(codepoint, glyph)| {
            (glyph.to_u32() == COLR_LINEAR_GLYPH)
                .then(|| char::from_u32(codepoint))
                .flatten()
        })
        .ok_or_else(|| "COLR fixture linear glyph has no Unicode mapping".into())
}

fn target_span(
    tree: &Tree,
) -> Result<(&usvg::Text, &usvg::layout::Span, usvg::fontdb::ID), String> {
    let Some(usvg::Node::Text(text)) = tree.node_by_id("target") else {
        return Err("variable fixture text missing".into());
    };
    let span = text
        .layouted()
        .first()
        .ok_or("variable fixture span missing")?;
    let id = span
        .positioned_glyphs
        .first()
        .ok_or("variable fixture glyph missing")?
        .font;
    Ok((text, span, id))
}

#[test]
fn static_font_without_axes_skips_variable_instance() -> Result<(), String> {
    let tree = fixture_tree(STATIC_FONT, None, 'A')?;
    let (text, span, id) = target_span(&tree)?;
    let (_, design) = html_glyph_variations::resolve_design(
        tree.fontdb(),
        id,
        &span.variations,
        span.font_optical_sizing,
        FONT_SIZE,
    )?;
    assert!(design.is_empty());
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    assert!(rasterizer.face_file(id)?.is_some());
    assert!(rasterizer.variable_instance(span, id, FONT_SIZE)?.is_none());
    assert!(rasterizer.primary_variable_face(text, id)?.is_none());
    Ok(())
}

#[test]
fn variation_resolution_rejects_invalid_size_and_missing_face() -> Result<(), String> {
    let tree = fixture_tree(STATIC_FONT, None, 'A')?;
    let (_, span, id) = target_span(&tree)?;
    let expected_size = format!("invalid variation font size for face {id:?}");
    for size in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let error = html_glyph_variations::resolve_design(
            tree.fontdb(),
            id,
            &span.variations,
            span.font_optical_sizing,
            size,
        )
        .err();
        assert_eq!(error.as_deref(), Some(expected_size.as_str()), "{size}");
    }
    let empty = Database::new();
    let error = html_glyph_variations::resolve_design(
        &empty,
        id,
        &span.variations,
        span.font_optical_sizing,
        FONT_SIZE,
    )
    .err();
    assert_eq!(
        error,
        Some(format!("font face data unavailable for {id:?}"))
    );
    Ok(())
}

#[test]
fn variable_colr_font_declines_skia_instance_for_color_tables() -> Result<(), String> {
    let character = colr_linear_character()?;
    let tree = fixture_tree(COLR_VARIABLE, Some("'SWPS' 10"), character)?;
    let (text, span, id) = target_span(&tree)?;
    let (_, design) = html_glyph_variations::resolve_design(
        tree.fontdb(),
        id,
        &span.variations,
        span.font_optical_sizing,
        FONT_SIZE,
    )?;
    assert!(!design.is_empty());
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    assert!(rasterizer.face_metadata_at(id, Some(&design))?.unsupported);
    assert!(rasterizer.variable_instance(span, id, FONT_SIZE)?.is_none());
    assert!(rasterizer.primary_variable_face(text, id)?.is_none());
    Ok(())
}

#[test]
fn variable_instance_propagates_invalid_font_size() -> Result<(), String> {
    let tree = fixture_tree(STATIC_FONT, None, 'A')?;
    let (_, span, id) = target_span(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let error = rasterizer.variable_instance(span, id, 0.0).err();
    assert_eq!(
        error,
        Some(format!("invalid variation font size for face {id:?}"))
    );
    assert!(rasterizer.variable_faces.borrow().instances.is_empty());
    Ok(())
}

#[test]
fn variable_svg_ignores_unknown_axis_and_normalizes_actual_design() -> Result<(), String> {
    let tree = fixture_tree(COLR_VARIABLE, Some("'SWPS' 10"), colr_linear_character()?)?;
    let (_, span, id) = target_span(&tree)?;
    let mut requested = span.variations.clone();
    assert!(requested.iter().any(|axis| axis.tag == *b"SWPS"));
    requested.push(usvg::FontVariation::new(*b"ZZZZ", 42.0));
    let (effective, design, normalized) = html_glyph_variations::resolve(
        tree.fontdb(),
        id,
        &requested,
        span.font_optical_sizing,
        FONT_SIZE,
    )?;
    assert!(effective.iter().any(|axis| axis.tag == *b"SWPS"));
    assert!(!effective.iter().any(|axis| axis.tag == *b"ZZZZ"));
    assert!(design.iter().any(|axis| axis.tag == *b"SWPS"));
    assert!(!design.iter().any(|axis| axis.tag == *b"ZZZZ"));
    assert!(!normalized.is_empty());
    assert!(
        normalized
            .iter()
            .all(|value| value.is_finite() && (-1.0..=1.0).contains(value))
    );
    Ok(())
}

#[test]
fn run_face_accepts_static_font_and_declines_variable_color_font() -> Result<(), String> {
    let static_tree = fixture_tree(STATIC_FONT, None, 'A')?;
    let (_, static_span, static_id) = target_span(&static_tree)?;
    let static_rasterizer = HtmlGlyphRasterizer::new(static_tree.fontdb());
    let (face, variable) = static_rasterizer
        .ensure_run_face(static_span, static_id, FONT_SIZE)?
        .ok_or("static text face was rejected")?;
    assert!(variable.is_none());
    assert!(face.upem > 0.0);

    let color_tree = fixture_tree(COLR_VARIABLE, Some("'SWPS' 10"), colr_linear_character()?)?;
    let (_, color_span, color_id) = target_span(&color_tree)?;
    let color_rasterizer = HtmlGlyphRasterizer::new(color_tree.fontdb());
    assert!(color_rasterizer.face_metadata(color_id)?.unsupported);
    let (_, design) = html_glyph_variations::resolve_design(
        color_tree.fontdb(),
        color_id,
        &color_span.variations,
        color_span.font_optical_sizing,
        FONT_SIZE,
    )?;
    assert!(!design.is_empty());
    assert!(
        color_rasterizer
            .ensure_run_face(color_span, color_id, FONT_SIZE)?
            .is_none()
    );
    Ok(())
}
