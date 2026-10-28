use super::{
    HtmlGlyphRasterizer, PaintCaptureScope, Prepared, PreparedSpan, anchor, assert_close,
    fixture_text, font_file_fixture, font_fixture, paint,
};
use resvg::usvg::{Options, Tree};
use std::sync::Arc;

const PAINT_SURFACE_WIDTH: i32 = 160;
const PAINT_SURFACE_HEIGHT: i32 = 80;

fn single_row_tree(
    fixture: &font_file_fixture::FontFileFixture,
    styled: bool,
) -> Result<Tree, String> {
    let style = if styled { "letter-spacing='1'" } else { "" };
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg'><text id='run' font-family='Noto Sans' font-size='11.48' transform='scale(0.99999434)'><tspan x='0' y='0' dy='1.1em' text-anchor='middle'><tspan>A</tspan><tspan {style}> system</tspan><tspan> of</tspan><tspan> the</tspan><tspan> bank.</tspan></tspan></text></svg>"
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
fn single_row_single_scalar_span_preserves_row_metrics_and_paint_size() -> Result<(), String> {
    let fixture = font_fixture("single-row-single-scalar")?;
    let tree = single_row_tree(&fixture, false)?;
    let text = fixture_text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("single row unavailable".into());
    };
    assert_eq!(text.chunks().len(), 1);
    assert_eq!(spans.len(), 5);
    assert_eq!(spans[0].glyphs.len(), 1);
    assert!(spans.iter().all(|span| span.chunk_alignment.is_some()));
    super::nested_rows::assert_row_metrics(&tree, text, &spans)?;
    let css = anchor::uniform_css_scale(text.abs_transform()).ok_or("CSS scale missing")?;
    for device_ratio in [1.0, 2.0] {
        assert_single_row_paint(&rasterizer, &spans, css, device_ratio)?;
    }
    Ok(())
}

fn assert_single_row_paint(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    spans: &[PreparedSpan],
    css: f32,
    device_ratio: f32,
) -> Result<(), String> {
    let mut surface =
        skia_safe::surfaces::raster_n32_premul((PAINT_SURFACE_WIDTH, PAINT_SURFACE_HEIGHT))
            .ok_or("surface unavailable")?;
    let capture = PaintCaptureScope::enter();
    for span in spans {
        let face = rasterizer
            .ensure_face(span.font)?
            .ok_or("face unavailable")?;
        paint::draw_span(
            surface.canvas(),
            span,
            face.typeface.as_skia()?,
            css * device_ratio,
        )?;
    }
    let snapshot = capture.snapshot();
    assert_eq!(snapshot.dropped, 0);
    assert_eq!(snapshot.events.len(), spans.len());
    for (span, event) in spans.iter().zip(snapshot.events) {
        assert_single_span_paint(span, &event, css, device_ratio)?;
    }
    Ok(())
}

fn assert_single_span_paint(
    span: &PreparedSpan,
    event: &serde_json::Value,
    css: f32,
    device_ratio: f32,
) -> Result<(), String> {
    let expected =
        anchor::paint_size(span.font_size, css, css).ok_or("paint size missing")? * device_ratio;
    assert_close(
        event["font"]["size"].as_f64().ok_or("paint size missing")?,
        f64::from(expected),
    );
    let first = span.glyphs.first().ok_or("glyph missing")?;
    let last = span.glyphs.last().ok_or("glyph missing")?;
    let source = last.transform().tx - first.transform().tx;
    let painted = paint::normalized_transform(span, last, css * device_ratio).tx
        - paint::normalized_transform(span, first, css * device_ratio).tx;
    assert_close(
        f64::from(painted / device_ratio),
        f64::from(source * expected / (device_ratio * span.font_size)),
    );
    Ok(())
}

#[test]
fn single_row_with_styled_span_is_excluded() -> Result<(), String> {
    let fixture = font_fixture("single-row-styled")?;
    let tree = single_row_tree(&fixture, true)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    if let Prepared::Ready(spans) = rasterizer.prepare(fixture_text(&tree)?)? {
        assert!(!spans.is_empty());
        assert!(spans.iter().all(|span| span.chunk_alignment.is_none()));
    }
    Ok(())
}
