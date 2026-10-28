use super::{
    HtmlGlyphRasterizer, LAYOUT_UNITS, Prepared, anchor, assert_close, fixture_text,
    font_file_fixture, font_fixture, paint,
};
use resvg::usvg::{Options, Tree};
use std::sync::Arc;

type SpanMeasurement = (Vec<(f32, f32)>, f64);

fn tree(fixture: &font_file_fixture::FontFileFixture, rows: &str) -> Result<Tree, String> {
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg'><text id='run' font-family='Noto Sans' font-size='11.48' transform='scale(0.99999434)'>{rows}</text></svg>"
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
fn unicode_single_and_multiple_rows_use_scalar_metrics_per_span() -> Result<(), String> {
    let fixture = font_fixture("unicode-spans")?;
    let first = "<tspan x='0' y='0' dy='1.1em' text-anchor='middle'><tspan>é</tspan><tspan xml:space='preserve'> café </tspan><tspan>été</tspan></tspan>";
    let second = "<tspan x='0' y='1em' dy='1.1em' text-anchor='middle'><tspan>à</tspan><tspan> voilà</tspan></tspan>";
    for rows in [first.to_owned(), format!("{first}{second}")] {
        let tree = tree(&fixture, &rows)?;
        let text = fixture_text(&tree)?;
        let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
        let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
            return Err("unicode fixture was not prepared".into());
        };
        assert_eq!(spans[0].glyphs.len(), 1);
        assert!(spans.iter().all(|span| span.chunk_alignment.is_some()));
        let css = anchor::uniform_css_scale(text.abs_transform()).ok_or("scale missing")?;
        let mut start = 0;
        for chunk in text.chunks() {
            let end = start + chunk.spans().len();
            let row = spans.get(start..end).ok_or("row spans missing")?;
            assert_row(&tree, chunk, row, css)?;
            start = end;
        }
        assert_eq!(start, spans.len());
    }
    Ok(())
}

fn assert_row(
    tree: &Tree,
    chunk: &resvg::usvg::TextChunk,
    spans: &[super::PreparedSpan],
    css: f32,
) -> Result<(), String> {
    let mut metrics = Vec::new();
    for (source, span) in chunk.spans().iter().zip(spans) {
        metrics.push(span_metrics(tree, chunk, source, span, css)?);
    }
    let mut cursor = f64::from(chunk.x().ok_or("anchor missing")? * css)
        - metrics.iter().map(|(_, width)| width).sum::<f64>() / 2.0;
    for (span, (values, width)) in spans.iter().zip(metrics) {
        assert_span(span, css, cursor, values[0].1)?;
        cursor += width;
    }
    Ok(())
}

fn span_metrics(
    tree: &Tree,
    chunk: &resvg::usvg::TextChunk,
    source: &resvg::usvg::TextSpan,
    span: &super::PreparedSpan,
    css: f32,
) -> Result<SpanMeasurement, String> {
    let content = chunk
        .text()
        .get(source.start()..source.end())
        .ok_or("invalid span boundary")?;
    let size = anchor::paint_size(span.font_size, css, css).ok_or("paint size missing")?;
    let font =
        super::super::super::text_shaping::FontShape::new(size, source.font().weight(), false);
    let values = super::super::super::text_shaping::shape_run_character_metrics_with_trim(
        tree.fontdb(),
        span.font,
        content,
        font,
        None,
    )
    .ok_or("span shape missing")?;
    assert_eq!(values.len(), content.chars().count());
    let width = values.iter().map(|v| f64::from(v.0)).sum::<f64>();
    Ok((values, (width * LAYOUT_UNITS).ceil() / LAYOUT_UNITS))
}

fn assert_span(
    span: &super::PreparedSpan,
    css: f32,
    cursor: f64,
    offset: f32,
) -> Result<(), String> {
    let first = span.glyphs.first().ok_or("glyph missing")?;
    let last = span.glyphs.last().ok_or("glyph missing")?;
    let expected =
        anchor::trunc_layout_unit((cursor + f64::from(offset)) as f32).ok_or("origin overflow")?;
    let used = anchor::paint_size(span.font_size, css, css).ok_or("paint size missing")?;
    for dpr in [1.0, 2.0] {
        let a = paint::normalized_transform(span, first, css * dpr);
        let b = paint::normalized_transform(span, last, css * dpr);
        assert_close(f64::from(a.tx / dpr), f64::from(expected));
        assert_close(
            f64::from((b.tx - a.tx) / dpr),
            f64::from((last.transform().tx - first.transform().tx) * used / span.font_size),
        );
    }
    Ok(())
}

#[test]
fn unsupported_unicode_and_mixed_styles_do_not_align() -> Result<(), String> {
    let fixture = font_fixture("unicode-exclusions")?;
    for content in [
        "<tspan>e&#x301;</tspan><tspan> café</tspan>",
        "<tspan>אב</tspan><tspan> café</tspan>",
        "<tspan>é</tspan><tspan font-family='serif'> café</tspan>",
        "<tspan>é</tspan><tspan font-size='12'> café</tspan>",
        "<tspan>é</tspan><tspan letter-spacing='1'> café</tspan>",
    ] {
        let tree = tree(
            &fixture,
            &format!("<tspan x='0' y='0' dy='1.1em' text-anchor='middle'>{content}</tspan>"),
        )?;
        let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
        if let Prepared::Ready(spans) = rasterizer.prepare(fixture_text(&tree)?)? {
            assert!(!spans.is_empty());
            assert!(
                spans.iter().all(|span| span.chunk_alignment.is_none()),
                "{content}"
            );
        }
    }
    Ok(())
}

#[test]
fn merged_cluster_is_not_admitted_as_a_unicode_scalar_mapping() -> Result<(), String> {
    let fixture = font_fixture("unicode-cluster-exclusion")?;
    let source = tree(
        &fixture,
        "<tspan x='0' y='0' dy='1.1em' text-anchor='middle'><tspan>é</tspan><tspan> café</tspan></tspan>",
    )?;
    let text = fixture_text(&source)?;
    let rasterizer = HtmlGlyphRasterizer::new(source.fontdb());
    let Prepared::Ready(mut spans) = rasterizer.prepare(text)? else {
        return Err("unicode fixture was not prepared".into());
    };
    for span in &mut spans {
        span.chunk_alignment = None;
    }
    spans[0].glyphs[0].text.push('i');
    super::super::chunk::apply(source.fontdb(), text, &mut spans)?;
    assert!(spans.iter().all(|span| span.chunk_alignment.is_none()));
    Ok(())
}
