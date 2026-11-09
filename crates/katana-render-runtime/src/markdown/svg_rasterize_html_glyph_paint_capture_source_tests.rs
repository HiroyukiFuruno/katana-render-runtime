use super::PreparedSpan;

pub(super) fn assert_identity_source(span: &PreparedSpan, event: &serde_json::Value) {
    let source = &event["sourceSpan"];
    assert_eq!(source["spanFontSize"], serde_json::json!(span.font_size));
    assert_eq!(source["rasterScale"], serde_json::json!(1.0));
    assert!(source["drawGlyphIndex"].is_null());
    assert_eq!(
        source["glyphs"].as_array().map(Vec::len),
        Some(span.glyphs.len())
    );
    for (index, glyph) in span.glyphs.iter().enumerate() {
        assert_eq!(
            source["glyphs"][index]["id"],
            serde_json::json!(span.glyph_ids[index])
        );
        assert_eq!(
            source["glyphs"][index]["fontSize"],
            serde_json::json!(glyph.font_size())
        );
    }
    assert_eq!(
        source["glyphs"][0]["relativePosition"],
        serde_json::json!([0.0, 0.0])
    );
}

pub(super) fn assert_transformed_source(index: usize, event: &serde_json::Value) {
    assert_eq!(
        event["sourceSpan"]["drawGlyphIndex"],
        serde_json::json!(index)
    );
    assert_eq!(
        event["glyphIds"][0],
        event["sourceSpan"]["glyphs"][index]["id"]
    );
}

#[test]
fn full_capture_drops_the_next_real_source_and_draw() -> Result<(), String> {
    let (span, typeface, _fixture) = super::prepared_fixture(false)?;
    let font = skia_safe::Font::new(typeface, 14.0);
    let scope = super::PaintCaptureScope::enter();
    for _ in 0..super::capture::MAX_EVENTS_FOR_TEST {
        super::record_test_event(&font, 0.0);
    }
    super::capture::record_source(&span, 1.0, None);
    super::record_test_event(&font, 0.0);
    let snapshot = scope.snapshot();
    assert_eq!(snapshot.events.len(), super::capture::MAX_EVENTS_FOR_TEST);
    assert_eq!(snapshot.dropped, 1);
    Ok(())
}

#[test]
fn rotated_real_text_selects_transformed_glyph_painting() -> Result<(), String> {
    let (span, typeface, _fixture) = super::prepared_fixture(true)?;
    let mut surface = skia_safe::surfaces::raster_n32_premul((128, 64))
        .ok_or("capture test surface unavailable")?;
    let scope = super::PaintCaptureScope::enter();
    super::super::draw_span(surface.canvas(), &span, &typeface, 1.0)?;
    let snapshot = scope.snapshot();
    assert!(!span.glyphs.is_empty());
    assert_eq!(snapshot.events.len(), span.glyphs.len());
    assert_eq!(snapshot.dropped, 0);
    for (index, event) in snapshot.events.iter().enumerate() {
        assert_transformed_source(index, event);
        super::assert_transformed_capture(event)?;
    }
    Ok(())
}

#[test]
fn font_instance_recorded_without_scope_does_not_leak_into_next_draw() -> Result<(), String> {
    let (span, typeface, _fixture) = super::prepared_fixture(false)?;
    super::capture::record_instance(&typeface, &[]);
    let mut surface = skia_safe::surfaces::raster_n32_premul((128, 64))
        .ok_or("capture test surface unavailable")?;
    let scope = super::PaintCaptureScope::enter();
    super::super::draw_span(surface.canvas(), &span, &typeface, 1.0)?;
    let snapshot = scope.snapshot();
    assert_eq!(snapshot.events.len(), 1);
    assert!(snapshot.events[0]["appliedVariationCoordinates"].is_null());
    assert_eq!(snapshot.dropped, 0);
    Ok(())
}
