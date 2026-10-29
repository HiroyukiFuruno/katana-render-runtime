use super::{
    HtmlGlyphRasterizer, LAYOUT_UNITS, Prepared, PreparedSpan, anchor, assert_close, fixture_text,
    font_file_fixture, font_fixture, paint,
};
use resvg::usvg::{Options, Tree};
use std::sync::Arc;

#[test]
fn nested_multi_span_rows_use_paint_size_for_each_relative_advance() -> Result<(), String> {
    let fixture = font_fixture("nested-rows")?;
    let tree = nested_rows_tree(&fixture, "", "")?;
    let text = fixture_text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("nested rows were not prepared".into());
    };
    assert!(
        spans.iter().all(|span| span.chunk_alignment.is_some()),
        "{}",
        super::super::chunk::row_diagnostic(text, &spans)
    );
    assert!(text.chunks().len() > 1);
    assert!(text.chunks().iter().any(|chunk| chunk.spans().len() > 1));
    assert!(spans.len() > text.chunks().len());
    let css_scale =
        anchor::uniform_css_scale(text.abs_transform()).ok_or("nested row CSS scale missing")?;
    let mut checked = 0;
    for span in &spans {
        checked += usize::from(assert_relative_advance(span, css_scale)?);
    }
    assert!(checked > 1, "multiple nonempty source runs must be checked");
    Ok(())
}

fn assert_relative_advance(span: &PreparedSpan, css_scale: f32) -> Result<bool, String> {
    let (Some(first), Some(last)) = (span.glyphs.first(), span.glyphs.last()) else {
        return Ok(false);
    };
    let source_advance = last.transform().tx - first.transform().tx;
    if source_advance == 0.0 {
        return Ok(false);
    }
    let paint_size = anchor::paint_size(span.font_size, css_scale, css_scale)
        .ok_or("nested row paint size missing")?;
    assert!(paint_size < span.font_size * css_scale);
    let expected = source_advance * paint_size / span.font_size;
    let first_paint = paint::normalized_transform(span, first, css_scale);
    let last_paint = paint::normalized_transform(span, last, css_scale);
    assert_close(
        f64::from(last_paint.tx - first_paint.tx),
        f64::from(expected),
    );
    Ok(true)
}

#[test]
fn nested_rows_with_letter_spacing_do_not_enter_alignment() -> Result<(), String> {
    let fixture = font_fixture("nested-rows-exclusion")?;
    let tree = nested_rows_tree(&fixture, "letter-spacing='1'", "")?;
    let text = fixture_text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    if let Prepared::Ready(spans) = rasterizer.prepare(text)? {
        assert!(!spans.is_empty());
        assert!(
            spans
                .iter()
                .all(|span| span.x_alignment.is_none() && span.chunk_alignment.is_none())
        );
    }
    Ok(())
}

pub(in crate::markdown::svg_rasterize::html_glyph) fn nested_rows_tree(
    fixture: &font_file_fixture::FontFileFixture,
    attributes: &str,
    row_attributes: &str,
) -> Result<Tree, String> {
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg'><text id='run' font-family='Noto Sans' font-size='11.48' transform='scale(0.99999434)' {attributes}><tspan x='0' y='0' dy='1.1em' text-anchor='middle'><tspan>A</tspan><tspan xml:space='preserve'>V customers </tspan></tspan><tspan x='0' y='1em' dy='1.1em' text-anchor='middle' {row_attributes}><tspan>information</tspan><tspan> about</tspan></tspan></text></svg>"
    );
    Tree::from_str(
        &svg,
        &Options {
            fontdb: Arc::new(fixture.database.clone()),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}

#[test]
fn nested_rows_keep_center_and_span_boundaries_from_span_metrics() -> Result<(), String> {
    let fixture = font_fixture("nested-row-centers")?;
    let tree = nested_rows_tree(&fixture, "", "")?;
    let text = fixture_text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("nested row centers were not prepared".into());
    };
    assert_row_metrics(&tree, text, &spans)
}

#[test]
fn adjacent_span_boundary_keeps_kerning_within_each_span() -> Result<(), String> {
    let fixture = font_fixture("nested-span-kerning")?;
    let tree = nested_rows_tree(&fixture, "", "")?;
    let text = fixture_text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("kerning fixture was not prepared".into());
    };
    assert_boundary_kerning(&tree, text, &spans)?;
    assert_row_metrics(&tree, text, &spans)
}

fn assert_boundary_kerning(
    tree: &Tree,
    text: &resvg::usvg::Text,
    spans: &[PreparedSpan],
) -> Result<(), String> {
    let row = text.chunks().first().ok_or("row missing")?;
    assert_eq!(row.text().get(..2), Some("AV"));
    let css = anchor::uniform_css_scale(text.abs_transform()).ok_or("CSS scale missing")?;
    let size = anchor::paint_size(spans[0].font_size, css, css).ok_or("paint size missing")?;
    let font = super::super::super::text_shaping::FontShape::new(
        size,
        row.spans()[0].font().weight(),
        false,
    );
    let shape = |text| {
        super::super::super::text_shaping::shape_run_character_metrics_with_trim(
            tree.fontdb(),
            spans[0].font,
            text,
            font,
            None,
        )
        .ok_or("kerning metrics missing")
    };
    let a = shape("A")?;
    let av = shape("AV")?;
    assert_ne!(a[0].0, av[0].0, "fixture must expose boundary kerning");
    Ok(())
}

pub(super) fn assert_row_metrics(
    tree: &Tree,
    text: &resvg::usvg::Text,
    spans: &[PreparedSpan],
) -> Result<(), String> {
    let css_scale = anchor::uniform_css_scale(text.abs_transform()).ok_or("CSS scale missing")?;
    let mut offset = 0;
    for chunk in text.chunks() {
        let end = offset + chunk.spans().len();
        let row = spans.get(offset..end).ok_or("row spans missing")?;
        let metrics = row_metrics(tree, chunk, row, css_scale)?;
        assert_row_positions(chunk, row, css_scale, &metrics)?;
        offset = end;
    }
    assert_eq!(offset, spans.len());
    Ok(())
}

fn row_metrics(
    tree: &Tree,
    chunk: &resvg::usvg::TextChunk,
    row: &[PreparedSpan],
    css_scale: f32,
) -> Result<Vec<(f32, f32)>, String> {
    let first = row.first().ok_or("row is empty")?;
    let source = chunk.spans().first().ok_or("row source missing")?;
    let size =
        anchor::paint_size(first.font_size, css_scale, css_scale).ok_or("paint size missing")?;
    let font = super::super::super::text_shaping::FontShape::new(
        size,
        source.font().weight(),
        source.font().style() != resvg::usvg::FontStyle::Normal,
    );
    shape_row_spans(tree, chunk, first, font)
}

fn shape_row_spans(
    tree: &Tree,
    chunk: &resvg::usvg::TextChunk,
    first: &PreparedSpan,
    font: super::super::super::text_shaping::FontShape,
) -> Result<Vec<(f32, f32)>, String> {
    let mut metrics = Vec::new();
    for source in chunk.spans() {
        let source_text = chunk
            .text()
            .get(source.start()..source.end())
            .ok_or("span text missing")?;
        let shaped = super::super::super::text_shaping::shape_run_character_metrics_with_trim(
            tree.fontdb(),
            first.font,
            source_text,
            font,
            None,
        )
        .ok_or("span shaping failed")?;
        metrics.extend(shaped);
    }
    Ok(metrics)
}

fn assert_row_positions(
    chunk: &resvg::usvg::TextChunk,
    row: &[PreparedSpan],
    css_scale: f32,
    metrics: &[(f32, f32)],
) -> Result<(), String> {
    let widths: Vec<f64> = chunk
        .spans()
        .iter()
        .map(|source| {
            let width: f64 = metrics[source.start()..source.end()]
                .iter()
                .map(|(advance, _)| f64::from(*advance))
                .sum();
            (width * LAYOUT_UNITS).ceil() / LAYOUT_UNITS
        })
        .collect();
    let mut cursor = f64::from(chunk.x().ok_or("row anchor missing")? * css_scale)
        - widths.iter().sum::<f64>() / 2.0;
    for ((source, span), width) in chunk.spans().iter().zip(row).zip(widths) {
        let glyph_offset = metrics.get(source.start()).ok_or("row offset missing")?.1;
        assert_glyph_position(span, css_scale, cursor, glyph_offset)?;
        cursor += width;
    }
    Ok(())
}

fn assert_glyph_position(
    span: &PreparedSpan,
    css_scale: f32,
    cursor: f64,
    glyph_offset: f32,
) -> Result<(), String> {
    let glyph = span.glyphs.first().ok_or("row glyph missing")?;
    let expected = anchor::trunc_layout_unit((cursor + f64::from(glyph_offset)) as f32)
        .ok_or("row origin overflow")?;
    for device_ratio in [1.0, 2.0] {
        let position = paint::normalized_transform(span, glyph, css_scale * device_ratio);
        assert_close(f64::from(position.tx / device_ratio), f64::from(expected));
        let expected_y =
            anchor::trunc_layout_unit((glyph.transform().ty + span.baseline_shift) * css_scale)
                .ok_or("row baseline overflow")?;
        assert_close(f64::from(position.ty / device_ratio), f64::from(expected_y));
    }
    Ok(())
}

#[test]
fn unsupported_later_row_does_not_partially_align_earlier_row() -> Result<(), String> {
    let fixture = font_fixture("nested-row-atomic")?;
    let tree = nested_rows_tree(&fixture, "", "letter-spacing='1'")?;
    let text = fixture_text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(mut spans) = rasterizer.prepare(text)? else {
        return Err("unsupported row fixture did not prepare".into());
    };
    assert!(spans.iter().all(|span| span.chunk_alignment.is_none()));
    super::super::chunk::apply(tree.fontdb(), text, &mut spans)?;
    assert!(spans.iter().all(|span| span.chunk_alignment.is_none()));
    Ok(())
}
