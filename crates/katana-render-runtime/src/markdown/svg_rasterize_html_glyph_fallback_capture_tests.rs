use super::super::super::{HtmlGlyphRasterizer, PaintCaptureScope, Prepared, PreparedSpan, paint};
use super::{TITLE, text, tree};
use serde_json::Value;

#[test]
fn fallback_paint_capture_records_run_alignment() -> Result<(), String> {
    let tree = tree(TITLE, "")?;
    let text = text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("fallback title was not prepared".into());
    };
    let fonts = spans
        .iter()
        .map(|span| span.font)
        .collect::<std::collections::HashSet<_>>();
    assert!(fonts.len() > 1, "expected multiple fallback faces");
    let mut surface = skia_safe::surfaces::raster_n32_premul((800, 160))
        .ok_or("fallback capture surface unavailable")?;
    let scope = PaintCaptureScope::enter();
    for span in &spans {
        draw_span(&rasterizer, surface.canvas(), span)?;
    }
    let snapshot = scope.snapshot();
    assert_eq!(snapshot.dropped, 0);
    assert_eq!(snapshot.events.len(), spans.len());
    for (span, event) in spans.iter().zip(&snapshot.events) {
        assert_fallback_alignment(span, event)?;
    }
    Ok(())
}

fn assert_fallback_alignment(span: &PreparedSpan, event: &Value) -> Result<(), String> {
    let alignment = span
        .fallback_alignment
        .as_ref()
        .ok_or("fallback alignment missing")?;
    assert_eq!(
        event["sourceSpan"]["fallbackAlignment"],
        serde_json::json!({
            "cssScale": alignment.css_scale,
            "paintCssSize": alignment.paint_css_size,
            "positionCount": alignment.css_positions.len(),
        })
    );
    Ok(())
}

#[test]
fn single_glyph_paint_capture_records_layout_anchor() -> Result<(), String> {
    let tree = tree("A", "")?;
    let text = text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("single glyph was not prepared".into());
    };
    let [span] = spans.as_slice() else {
        return Err("expected one run".into());
    };
    let mut surface =
        skia_safe::surfaces::raster_n32_premul((800, 160)).ok_or("capture surface unavailable")?;
    let scope = PaintCaptureScope::enter();
    draw_span(&rasterizer, surface.canvas(), span)?;
    let snapshot = scope.snapshot();
    assert_eq!(snapshot.dropped, 0);
    assert_eq!(snapshot.events.len(), 1);
    assert_layout_anchor(span, &snapshot.events[0])
}

fn assert_layout_anchor(span: &PreparedSpan, event: &Value) -> Result<(), String> {
    let alignment = span.x_alignment.ok_or("x alignment missing")?;
    let layout = alignment.layout.ok_or("layout anchor missing")?;
    assert_eq!(
        event["sourceSpan"]["xAlignment"]["layout"],
        serde_json::json!({
            "width": layout.width,
            "anchor": format!("{:?}", layout.anchor),
            "cssScale": layout.css_scale,
        })
    );
    Ok(())
}

fn draw_span(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    canvas: &skia_safe::Canvas,
    span: &PreparedSpan,
) -> Result<(), String> {
    let face = rasterizer.ensure_face(span.font)?.ok_or("face missing")?;
    paint::draw_span(canvas, span, face.typeface.as_skia(), 1.0)
}
