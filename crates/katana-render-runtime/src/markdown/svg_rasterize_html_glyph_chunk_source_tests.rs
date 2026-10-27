use super::{CSS_SCALE, OBSERVED_BASELINE, OBSERVED_ORIGINS, POSITION_TOLERANCE};
use crate::markdown::svg_rasterize::html_glyph::{
    HtmlGlyphRasterizer, Prepared, PreparedSpan, paint,
};
use resvg::usvg::{Node, Options, Tree};

const SPLIT_CONTENT: &str = "<tspan>Single</tspan><tspan> Page</tspan><tspan> Application</tspan>";
const TEXT_CONTENT: &str = "Single Page Application";
const EXPECTED_SOURCE_SPAN_COUNT: usize = 3;
const EXPLICIT_TEXT_LENGTH: f32 = 180.0;

#[path = "svg_rasterize_html_glyph_chunk_property_tests.rs"]
mod properties;

#[test]
fn source_span_layout_units_match_observed_middle_origins() -> Result<(), String> {
    let spans = prepared(SPLIT_CONTENT, "", CSS_SCALE)?;
    assert_origins(&spans, OBSERVED_ORIGINS)
}

#[test]
fn chunk_anchor_preserves_authored_x_before_origin_truncation() -> Result<(), String> {
    let content = "<tspan x='9'>Single</tspan><tspan> Page</tspan><tspan> Application</tspan>";
    let spans = prepared(content, "", CSS_SCALE)?;
    assert_origins(&spans, [-70.421_875, -28.437_5, 8.875])
}

#[test]
fn trailing_space_advance_participates_in_chunk_anchor() -> Result<(), String> {
    let content = "<tspan>Single</tspan><tspan> Page</tspan><tspan xml:space='preserve'> Application </tspan>";
    let spans = prepared(content, "", CSS_SCALE)?;
    let last = spans
        .last()
        .and_then(|span| span.glyphs.last())
        .ok_or("trailing glyph is missing")?;
    assert_eq!(last.text, " ");
    assert_origins(&spans, [-81.375, -39.390_625, -2.062_5])
}

fn assert_origins(
    spans: &[PreparedSpan],
    expected_origins: [f32; EXPECTED_SOURCE_SPAN_COUNT],
) -> Result<(), String> {
    assert_eq!(spans.len(), expected_origins.len());
    for (span, expected) in spans.iter().zip(expected_origins) {
        let glyph = span.glyphs.first().ok_or("source span has no glyph")?;
        let transform = paint::normalized_transform(span, glyph, CSS_SCALE);
        assert!(span.x_alignment.is_none());
        assert!(span.chunk_alignment.is_some());
        assert!((transform.tx - expected).abs() < POSITION_TOLERANCE);
        assert_eq!(transform.ty, OBSERVED_BASELINE);
    }
    Ok(())
}

#[test]
fn single_middle_quantizes_origin_and_nominal_size_keeps_existing_alignment() -> Result<(), String>
{
    let single = prepared(TEXT_CONTENT, "", CSS_SCALE)?;
    assert_eq!(single.len(), 1);
    assert!(
        single
            .iter()
            .all(|span| span.x_alignment.is_none() && span.chunk_alignment.is_some())
    );
    let span = single.first().ok_or("single middle span is missing")?;
    let glyph = span
        .glyphs
        .first()
        .ok_or("single middle glyph is missing")?;
    assert_eq!(
        paint::normalized_transform(span, glyph, CSS_SCALE).tx,
        OBSERVED_ORIGINS[0]
    );
    let nominal = prepared(SPLIT_CONTENT, "", 1.0)?;
    assert!(nominal.iter().all(|span| span.chunk_alignment.is_none()));
    Ok(())
}

#[test]
fn text_length_exclusion_uses_the_retained_source_span_property() -> Result<(), String> {
    assert_outer_text_length_model()?;
    let content =
        "<tspan textLength='180'>Single</tspan><tspan> Page</tspan><tspan> Application</tspan>";
    let source = tree(content, "", CSS_SCALE)?;
    let Some(Node::Text(text)) = source.node_by_id("run") else {
        return Err("tspan textLength source text is missing".to_owned());
    };
    let chunk = text
        .chunks()
        .first()
        .ok_or("tspan textLength chunk is missing")?;
    let span = chunk
        .spans()
        .first()
        .ok_or("tspan textLength span is missing")?;
    assert_eq!(span.text_length(), Some(EXPLICIT_TEXT_LENGTH));
    assert_excluded(content, "")
}

fn assert_outer_text_length_model() -> Result<(), String> {
    let source = tree(SPLIT_CONTENT, "textLength='180'", CSS_SCALE)?;
    let Some(Node::Text(text)) = source.node_by_id("run") else {
        return Err("outer textLength source text is missing".to_owned());
    };
    /* WHY: usvgは直親tspanのtextLengthだけを保持するため、outer属性の伝播を仮定しない。 */
    assert!(
        text.chunks()
            .iter()
            .flat_map(|chunk| chunk.spans())
            .all(|span| span.text_length().is_none())
    );
    Ok(())
}

#[test]
fn decorated_chunk_keeps_the_existing_skia_unsupported_contract() -> Result<(), String> {
    let source = tree(SPLIT_CONTENT, "text-decoration='underline'", CSS_SCALE)?;
    assert!(matches!(prepare_tree(&source)?, Prepared::Unsupported));
    Ok(())
}

#[test]
fn automatic_css_weight_axis_remains_eligible() -> Result<(), String> {
    let source = tree(SPLIT_CONTENT, "", CSS_SCALE)?;
    let Some(Node::Text(text)) = source.node_by_id("run") else {
        return Err("CSS weight source text is missing".to_owned());
    };
    let chunk = text.chunks().first().ok_or("CSS weight chunk is missing")?;
    let span = chunk.spans().first().ok_or("CSS weight span is missing")?;
    let [axis] = span.font().variations() else {
        return Err("CSS weight did not produce one automatic axis".to_owned());
    };
    assert_eq!(axis.tag, *b"wght");
    assert_eq!(axis.value, f32::from(span.font().weight()));
    assert!(span.baseline_shift().is_empty());
    assert!(super::super::eligibility::input(text, &prepared_tree(&source)?).is_some());
    Ok(())
}

#[test]
fn mixed_sizes_and_faces_exclude_chunk_alignment() -> Result<(), String> {
    for content in [
        "<tspan>Single</tspan><tspan font-size='18'> Page</tspan>",
        "<tspan>Single</tspan><tspan font-family='Trebuchet MS'> Page</tspan>",
        "<tspan x='0'>Single</tspan><tspan x='20'> Page</tspan>",
    ] {
        assert_excluded(content, "")?;
    }
    Ok(())
}

#[test]
fn merged_scalar_clusters_exclude_chunk_alignment() -> Result<(), String> {
    let source = tree(SPLIT_CONTENT, "", CSS_SCALE)?;
    let Some(Node::Text(text)) = source.node_by_id("run") else {
        return Err("scalar mapping source text is missing".to_owned());
    };
    let mut spans = prepared_tree(&source)?;
    let first = spans.first_mut().ok_or("first scalar span is missing")?;
    let glyph = first
        .glyphs
        .first_mut()
        .ok_or("first scalar glyph is missing")?;
    glyph.text.push('i');
    assert!(super::super::eligibility::input(text, &spans).is_none());
    Ok(())
}

#[test]
fn eligible_chunk_metric_failure_is_returned_to_the_caller() -> Result<(), String> {
    let source = tree(SPLIT_CONTENT, "", CSS_SCALE)?;
    let Some(Node::Text(text)) = source.node_by_id("run") else {
        return Err("metric failure source text is missing".to_owned());
    };
    let mut spans = prepared_tree(&source)?;
    let unavailable = resvg::usvg::fontdb::Database::new();
    assert_eq!(
        super::super::apply(&unavailable, text, &mut spans),
        Err("eligible SVG chunk character metrics could not be shaped".to_owned())
    );
    Ok(())
}

fn assert_excluded(content: &str, attributes: &str) -> Result<(), String> {
    let source = tree(content, attributes, CSS_SCALE)?;
    if let Prepared::Ready(spans) = prepare_tree(&source)? {
        assert!(
            spans.iter().all(|span| span.chunk_alignment.is_none()),
            "{content} {attributes}"
        );
    }
    Ok(())
}

fn prepared(content: &str, attributes: &str, scale: f32) -> Result<Vec<PreparedSpan>, String> {
    let tree = tree(content, attributes, scale)?;
    prepared_tree(&tree)
}

fn prepared_tree(tree: &Tree) -> Result<Vec<PreparedSpan>, String> {
    match prepare_tree(tree)? {
        Prepared::Ready(spans) => Ok(spans),
        Prepared::Unsupported => Err("chunk source is not supported by Skia".to_owned()),
    }
}

fn prepare_tree(tree: &Tree) -> Result<Prepared, String> {
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("chunk source text is missing".to_owned());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    rasterizer.prepare(text)
}

fn tree(content: &str, attributes: &str, scale: f32) -> Result<Tree, String> {
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg'><text id='run' x='0' y='14' \
         font-family='Helvetica' font-weight='700' font-size='14' text-anchor='middle' \
         transform='scale({scale})' {attributes}>{content}</text></svg>"
    );
    let options = Options {
        fontdb: crate::markdown::svg_rasterize::font::html_font_db_for_text(
            "Helvetica, Trebuchet MS",
            content,
        ),
        ..Options::default()
    };
    Tree::from_str(&svg, &options).map_err(|error| error.to_string())
}
