use super::nested_rows_fixture::{NestedRowsSpec, nested_rows_tree_with_anchor};
use super::{
    HtmlGlyphRasterizer, Prepared, PreparedSpan, anchor, assert_close, fixture_text, font_fixture,
    paint,
};
use resvg::usvg::{TextChunk, Tree};

const FONT_SIZE: &str = "16";
const CSS_TRANSFORM: &str = "scale(0.999987675)";
const ANCHOR_X: &str = "216";
const SINGLE_ROW_COUNT: usize = 1;
const MULTI_ROW_COUNT: usize = 2;

#[test]
fn start_anchor_single_row_with_nested_source_spans_aligns() -> Result<(), String> {
    assert_start_anchor_case("start-anchor-single-row", false)
}

#[test]
fn start_anchor_multiple_rows_with_nested_source_spans_aligns() -> Result<(), String> {
    assert_start_anchor_case("start-anchor-multiple-rows", true)
}

fn assert_start_anchor_case(name: &str, second_row: bool) -> Result<(), String> {
    let fixture = font_fixture(name)?;
    let spec = NestedRowsSpec {
        anchor: "start",
        font_size: FONT_SIZE,
        transform: CSS_TRANSFORM,
        anchor_x: ANCHOR_X,
        second_row,
    };
    let tree =
        nested_rows_tree_with_anchor(&fixture, "style='text-align:left !important'", "", spec)?;
    let text = fixture_text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("start-anchor nested spans were not prepared".into());
    };
    let expected_rows = if second_row {
        MULTI_ROW_COUNT
    } else {
        SINGLE_ROW_COUNT
    };
    assert_row_alignments(&tree, text, &spans, expected_rows)
}

fn assert_row_alignments(
    tree: &Tree,
    text: &resvg::usvg::Text,
    spans: &[PreparedSpan],
    expected_rows: usize,
) -> Result<(), String> {
    assert_eq!(text.chunks().len(), expected_rows);
    let css_scale = anchor::uniform_css_scale(text.abs_transform()).ok_or("scale missing")?;
    let mut offset = 0;
    for chunk in text.chunks() {
        assert_eq!(chunk.anchor(), resvg::usvg::TextAnchor::Start);
        assert!(chunk.spans().len() > 1);
        let end = offset + chunk.spans().len();
        let row = spans.get(offset..end).ok_or("source row missing")?;
        assert!(row.iter().all(|span| span.chunk_alignment.is_some()));
        assert_row_positions(tree, chunk, row, css_scale)?;
        offset = end;
    }
    assert_eq!(offset, spans.len());
    Ok(())
}

fn assert_row_positions(
    tree: &Tree,
    chunk: &TextChunk,
    row: &[PreparedSpan],
    css_scale: f32,
) -> Result<(), String> {
    let metrics = super::nested_rows::row_metrics(tree, chunk, row, css_scale)?;
    let origin = f64::from(chunk.x().ok_or("start anchor missing")? * css_scale);
    let first_source = chunk.spans().first().ok_or("start source span missing")?;
    let first_metrics = source_span_metrics(chunk, first_source, &metrics)?;
    assert_anchor_origin(row, first_metrics, origin, css_scale)?;
    let mut cursor = origin;
    for (source, span) in chunk.spans().iter().zip(row) {
        let content = chunk
            .text()
            .get(source.start()..source.end())
            .ok_or("source span boundary is invalid")?;
        let glyph_text: String = span
            .glyphs
            .iter()
            .map(|glyph| glyph.text.as_str())
            .collect();
        assert_eq!(glyph_text, content, "source span boundary changed");
        let span_metrics = source_span_metrics(chunk, source, &metrics)?;
        cursor = assert_span_origin_and_width(span, span_metrics, cursor, css_scale)?;
    }
    Ok(())
}

fn source_span_metrics<'a>(
    chunk: &TextChunk,
    source: &resvg::usvg::TextSpan,
    metrics: &'a [(f32, f32)],
) -> Result<&'a [(f32, f32)], String> {
    let text = chunk.text();
    let prefix = text
        .get(..source.start())
        .ok_or("source span starts inside UTF-8 scalar")?
        .chars()
        .count();
    let content = text
        .get(source.start()..source.end())
        .ok_or("source span boundary is invalid")?;
    let end = prefix + content.chars().count();
    metrics
        .get(prefix..end)
        .ok_or_else(|| "source span metrics are missing".to_owned())
}

fn assert_anchor_origin(
    row: &[PreparedSpan],
    metrics: &[(f32, f32)],
    origin: f64,
    css_scale: f32,
) -> Result<(), String> {
    let first_span = row.first().ok_or("start row is empty")?;
    let first_metric = metrics.first().ok_or("start row metrics missing")?;
    assert_close(f64::from(first_metric.1), 0.0);
    let expected = anchor::trunc_layout_unit(origin as f32).ok_or("anchor overflow")?;
    let glyph = first_span.glyphs.first().ok_or("start glyph missing")?;
    let actual = paint::normalized_transform(first_span, glyph, css_scale).tx;
    assert_close(f64::from(actual), f64::from(expected));
    Ok(())
}

fn assert_span_origin_and_width(
    span: &PreparedSpan,
    metrics: &[(f32, f32)],
    cursor: f64,
    css_scale: f32,
) -> Result<f64, String> {
    assert_eq!(metrics.len(), span.glyphs.len());
    let first = span.glyphs.first().ok_or("start span is empty")?;
    let first_offset = metrics.first().ok_or("start span metrics missing")?.1;
    let expected_origin = anchor::trunc_layout_unit((cursor + f64::from(first_offset)) as f32)
        .ok_or("start origin overflow")?;
    let actual_origin = paint::normalized_transform(span, first, css_scale).tx;
    assert_close(f64::from(actual_origin), f64::from(expected_origin));
    /* WHY: LayoutUnitはsource span原点だけで適用し、run内advanceを二重に切り捨てない。 */
    let width: f64 = metrics.iter().map(|(advance, _)| f64::from(*advance)).sum();
    Ok(cursor + super::super::chunk::ceil_layout_unit(width))
}
