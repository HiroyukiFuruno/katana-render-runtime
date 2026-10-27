use super::super::{HtmlGlyphRasterizer, Prepared, PreparedSpan};
use super::{
    PaintCaptureScope, capture, draw_identity_glyphs, draw_transformed_glyphs, text_font,
    text_paint,
};
use resvg::usvg::{Node, Options, Tree};
use skia_safe::{Data, Font, FontMgr, M44, Point, Typeface, surfaces};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const FAMILY: &str = "Noto Sans";
const SAMPLE: &str = "AV";
static TEMP_FONT_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[test]
fn paint_capture_records_actual_identity_draw_arguments() -> Result<(), String> {
    let (span, snapshot) = capture_fixture_draw(false)?;
    assert_identity_capture(&span, &snapshot)
}

#[test]
fn paint_capture_report_serializes_empty_and_actual_draw_snapshots() -> Result<(), String> {
    let (_, captured) = capture_fixture_draw(false)?;
    for (snapshot, status) in [
        (capture::Snapshot::default(), "empty"),
        (captured, "captured"),
    ] {
        let report = snapshot.report_value();
        assert_eq!(report["status"], status);
        assert_eq!(report["rows"], serde_json::json!(snapshot.events));
        assert_eq!(report["capturedDrawCount"], snapshot.events.len());
        assert_eq!(report["hasRows"], !snapshot.events.is_empty());
        assert_eq!(report["dropped"], 0);
        assert_eq!(report["complete"], true);
    }
    Ok(())
}

fn assert_identity_capture(
    span: &PreparedSpan,
    snapshot: &capture::Snapshot,
) -> Result<(), String> {
    assert_eq!(snapshot.dropped, 0);
    assert_eq!(snapshot.events.len(), 1);
    let event = &snapshot.events[0];
    let glyph_ids = event["glyphIds"]
        .as_array()
        .ok_or("captured glyph IDs missing")?;
    let positions = event["positions"]
        .as_array()
        .ok_or("captured positions missing")?;
    assert_eq!(glyph_ids.len(), span.glyph_ids.len());
    assert_eq!(positions.len(), glyph_ids.len());
    assert_eq!(event["origin"], serde_json::json!([0.0, 0.0]));
    assert_eq!(
        event["localToDeviceColumnMajor"].as_array().map(Vec::len),
        Some(16)
    );
    assert_recorded_font(event)
}

fn assert_recorded_font(event: &serde_json::Value) -> Result<(), String> {
    assert_eq!(event["font"]["size"].as_f64(), Some(14.0));
    assert!(
        event["typefaceFamily"]
            .as_str()
            .is_some_and(|family| !family.is_empty())
    );
    assert!(event["font"]["scaleX"].is_number());
    assert!(event["font"]["skewX"].is_number());
    assert!(event["font"]["hinting"].is_string());
    assert!(event["font"]["edging"].is_string());
    for flag in [
        "subpixel",
        "linearMetrics",
        "baselineSnap",
        "embeddedBitmaps",
        "embolden",
        "forceAutoHinting",
    ] {
        assert!(event["font"][flag].is_boolean());
    }
    Ok(())
}

#[test]
fn paint_capture_records_transformed_draw_local_to_device_matrix() -> Result<(), String> {
    let (span, snapshot) = capture_fixture_draw(true)?;
    assert_eq!(snapshot.dropped, 0);
    assert_eq!(snapshot.events.len(), span.glyph_ids.len());
    assert!(snapshot.events.len() > 1);
    for event in snapshot.events {
        assert_transformed_capture(&event)?;
    }
    Ok(())
}

fn assert_transformed_capture(event: &serde_json::Value) -> Result<(), String> {
    assert_eq!(event["glyphIds"].as_array().map(Vec::len), Some(1));
    assert_eq!(event["positions"].as_array().map(Vec::len), Some(1));
    let matrix = event["localToDeviceColumnMajor"]
        .as_array()
        .ok_or("transformed matrix missing")?;
    assert_eq!(matrix.len(), 16);
    assert!(matrix[1] != serde_json::json!(0.0) || matrix[4] != serde_json::json!(0.0));
    Ok(())
}

#[test]
fn paint_capture_scope_restores_nested_state_and_does_not_leak() -> Result<(), String> {
    let typeface = bundled_typeface()?;
    let font = Font::new(typeface, 14.0);
    let outer = PaintCaptureScope::enter();
    record_test_event(&font, 1.0);
    assert_eq!(outer.snapshot().events.len(), 1);
    assert_nested_scope_restoration(&font, &outer);
    drop(outer);
    assert_scope_restarts_empty(&font);
    Ok(())
}

fn assert_nested_scope_restoration(font: &Font, outer: &PaintCaptureScope) {
    let inner = PaintCaptureScope::enter();
    record_test_event(font, 2.0);
    assert_eq!(inner.snapshot().events.len(), 1);
    drop(inner);
    assert_eq!(outer.snapshot().events.len(), 1);
}

fn assert_scope_restarts_empty(font: &Font) {
    record_test_event(font, 3.0);
    let next = PaintCaptureScope::enter();
    assert!(next.snapshot().events.is_empty());
    record_test_event(font, 4.0);
    assert_eq!(next.snapshot().events.len(), 1);
    assert_eq!(next.snapshot().dropped, 0);
}

#[test]
fn paint_capture_bounds_event_and_glyph_counts() -> Result<(), String> {
    let typeface = bundled_typeface()?;
    let font = Font::new(typeface, 14.0);
    assert_event_limit(&font);
    assert_glyph_limit(&font);
    Ok(())
}

fn assert_event_limit(font: &Font) {
    let scope = PaintCaptureScope::enter();
    for _ in 0..capture::MAX_EVENTS_FOR_TEST {
        record_test_event(font, 0.0);
    }
    assert_eq!(scope.snapshot().events.len(), capture::MAX_EVENTS_FOR_TEST);
    assert_eq!(scope.snapshot().dropped, 0);
    record_test_event(font, 0.0);
    assert_eq!(scope.snapshot().events.len(), capture::MAX_EVENTS_FOR_TEST);
    assert_eq!(scope.snapshot().dropped, 1);
    let report = scope.snapshot().report_value();
    assert_eq!(report["status"], "overflow");
    assert_eq!(report["dropped"], 1);
    assert_eq!(report["complete"], false);
}

fn assert_glyph_limit(font: &Font) {
    let oversized = vec![1_u16; capture::MAX_GLYPHS_PER_EVENT_FOR_TEST + 1];
    let oversized_positions = vec![Point::new(0.0, 0.0); oversized.len()];
    let oversized_scope = PaintCaptureScope::enter();
    capture::record_draw(
        font,
        &oversized,
        &oversized_positions,
        Point::new(0.0, 0.0),
        M44::default(),
    );
    assert!(oversized_scope.snapshot().events.is_empty());
    assert_eq!(oversized_scope.snapshot().dropped, 1);
}

fn prepared_fixture(
    transformed: bool,
) -> Result<
    (
        PreparedSpan,
        Typeface,
        crate::markdown::svg_rasterize::font_file_fixture::FontFileFixture,
    ),
    String,
> {
    let fixture = crate::markdown::svg_rasterize::font_file_fixture::create(
        temp_font_path(),
        crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    let tree = parse_fixture_tree(&fixture, transformed)?;
    let (span, typeface) = prepare_first_span(&tree)?;
    Ok((span, typeface, fixture))
}

fn parse_fixture_tree(
    fixture: &crate::markdown::svg_rasterize::font_file_fixture::FontFileFixture,
    transformed: bool,
) -> Result<Tree, String> {
    let transform = if transformed {
        r#" rotate="12 12""#
    } else {
        ""
    };
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="capture" x="10" y="24" font-family="{FAMILY}" font-size="14"{transform}>{SAMPLE}</text></svg>"#
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

fn prepare_first_span(tree: &Tree) -> Result<(PreparedSpan, Typeface), String> {
    let Some(Node::Text(text)) = tree.node_by_id("capture") else {
        return Err("capture fixture text missing".into());
    };
    let mut rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    rasterizer.backend = super::super::RasterBackend::Skia;
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("capture fixture text was not prepared".into());
    };
    let span = spans
        .into_iter()
        .next()
        .ok_or("capture fixture span missing")?;
    let face = rasterizer
        .ensure_face(span.font)?
        .ok_or("capture fixture face unavailable")?;
    let typeface = face.typeface.as_skia()?.clone();
    Ok((span, typeface))
}

fn capture_fixture_draw(transformed: bool) -> Result<(PreparedSpan, capture::Snapshot), String> {
    let (span, typeface, _fixture) = prepared_fixture(transformed)?;
    let mut surface =
        surfaces::raster_n32_premul((128, 64)).ok_or("capture test surface unavailable")?;
    let scope = PaintCaptureScope::enter();
    let font = text_font(&span, &typeface, 1.0);
    let paint = text_paint(&span);
    if transformed {
        draw_transformed_glyphs(surface.canvas(), &span, &font, &paint, 1.0)?;
    } else {
        draw_identity_glyphs(surface.canvas(), &span, &font, &paint, 1.0)?;
    }
    Ok((span, scope.snapshot()))
}

fn bundled_typeface() -> Result<Typeface, String> {
    FontMgr::default()
        .new_from_data(
            Data::new_copy(crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES),
            Some(0),
        )
        .ok_or_else(|| "bundled capture test typeface unavailable".to_owned())
}

fn record_test_event(font: &Font, coordinate: f32) {
    let glyph_ids = [1_u16];
    let positions = [Point::new(coordinate, coordinate + 1.0)];
    capture::record_draw(
        font,
        &glyph_ids,
        &positions,
        Point::new(coordinate + 2.0, coordinate + 3.0),
        M44::default(),
    );
}

fn temp_font_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "krr-glyph-paint-capture-{}-{}.ttf",
        std::process::id(),
        TEMP_FONT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}
