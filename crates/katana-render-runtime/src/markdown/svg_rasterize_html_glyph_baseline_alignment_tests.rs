use super::{PolicyKind, kind};
use crate::markdown::svg_rasterize::{
    font::html_font_db_for_text, html_glyph::HtmlGlyphRasterizer,
};
use resvg::usvg::{Node, Options, Tree};

const SEQUENCE_SOURCE_Y: f32 = 89.0;
const SEQUENCE_SOURCE_X: f32 = 196.0;
const SEQUENCE_DY_EM: f32 = 1.0;
const SEQUENCE_FONT_SIZE: f32 = 16.0;
const SEQUENCE_CSS_SCALE: f32 = 0.925_558_3;
const SEQUENCE_CHROME_ORIGIN: f32 = 101.046_875;
const ORIGIN_TOLERANCE: f32 = 0.0001;
const FONT_FAMILY: &str = "Trebuchet MS";
const SEQUENCE_SAMPLE: &str = "Hello";

#[test]
fn explicit_alignment_middle_uses_sequence_chrome_origin() -> Result<(), String> {
    let actual = snapped_sequence_origin()?;
    assert!(
        (actual - SEQUENCE_CHROME_ORIGIN).abs() < ORIGIN_TOLERANCE,
        "sequence origin {actual}, expected {SEQUENCE_CHROME_ORIGIN}"
    );
    Ok(())
}

#[test]
fn explicit_and_dominant_middle_keep_distinct_policy_kinds() -> Result<(), String> {
    let explicit = sequence_tree(true)?;
    let dominant = sequence_tree(false)?;
    assert_eq!(source_middle_kind(&explicit)?, PolicyKind::AlignmentMiddle);
    assert_eq!(source_middle_kind(&dominant)?, PolicyKind::Middle);
    Ok(())
}

fn snapped_sequence_origin() -> Result<f32, String> {
    let tree = sequence_tree(true)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("sequence text missing".to_owned());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let policy = super::resolved_policy(&rasterizer, text)?
        .ok_or("sequence alignment-middle policy rejected".to_owned())?;
    let super::super::Prepared::Ready(mut spans) = rasterizer.prepare(text)? else {
        return Err("sequence text was not prepared".to_owned());
    };
    let span = spans.pop().ok_or("sequence glyph span missing")?;
    let glyph = span.glyphs.first().ok_or("sequence glyph missing")?;
    let scale = policy.css_scale.ok_or("sequence CSS scale missing")?;
    let origin = super::super::paint::normalized_transform(&span, glyph, scale).ty;
    /* WHY: これはCSS LayoutUnit化後のorigin投影で、最終paint PNG値ではない。 */
    super::super::anchor::trunc_layout_unit(origin)
        .ok_or("sequence origin could not be layout-unit snapped".to_owned())
}

fn source_middle_kind(tree: &Tree) -> Result<PolicyKind, String> {
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("sequence text missing".to_owned());
    };
    let span = text
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .ok_or("sequence source span missing".to_owned())?;
    kind::span_kind(span, true).ok_or("sequence middle kind unsupported".to_owned())
}

fn sequence_tree(explicit_alignment: bool) -> Result<Tree, String> {
    let alignment = if explicit_alignment {
        " alignment-baseline=\"middle\""
    } else {
        ""
    };
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" x="{SEQUENCE_SOURCE_X}" y="{SEQUENCE_SOURCE_Y}" dy="{SEQUENCE_DY_EM}em" transform="scale({SEQUENCE_CSS_SCALE})" font-size="{SEQUENCE_FONT_SIZE}" font-family="{FONT_FAMILY}" text-anchor="middle" dominant-baseline="middle"{alignment}>{SEQUENCE_SAMPLE}</text></svg>"#
    );
    let database = html_font_db_for_text(FONT_FAMILY, SEQUENCE_SAMPLE);
    Tree::from_str(
        &source,
        &Options {
            fontdb: database,
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}
