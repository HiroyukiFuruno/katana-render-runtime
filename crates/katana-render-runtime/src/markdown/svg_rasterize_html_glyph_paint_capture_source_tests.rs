use super::PreparedSpan;

#[cfg(target_os = "macos")]
const FIRST_CAPTURE_WEIGHT: f32 = 400.0;
#[cfg(target_os = "macos")]
const REPLACEMENT_WEIGHT: f32 = 350.0;
#[cfg(target_os = "macos")]
const WEIGHT_AXIS: [u8; 4] = *b"wght";

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

#[cfg(target_os = "macos")]
#[test]
fn full_instance_capture_keeps_existing_face_and_drops_new_face() -> Result<(), String> {
    let base = variable_typeface()?;
    let scope = super::PaintCaptureScope::enter();
    let first = fill_instance_capture(&base)?;
    let replacement = [resvg::usvg::FontVariation::new(
        WEIGHT_AXIS,
        REPLACEMENT_WEIGHT,
    )];
    super::capture::record_instance(&first, &replacement);
    let font = skia_safe::Font::new(first, 14.0);
    super::record_test_event(&font, 0.0);
    let before = scope.snapshot();
    assert_eq!(before.dropped, 0);
    assert_eq!(
        before.events[0]["appliedVariationCoordinates"][0]["value"],
        FIRST_CAPTURE_WEIGHT
    );
    let overflow_weight = FIRST_CAPTURE_WEIGHT + super::capture::MAX_EVENTS_FOR_TEST as f32;
    let new_face = variable_instance(&base, overflow_weight)?;
    super::capture::record_instance(&new_face, &[]);
    let after = scope.snapshot();
    assert_eq!(after.events, before.events);
    assert_eq!(after.dropped, 1);
    assert_eq!(after.report_value()["status"], "overflow");
    assert_eq!(after.report_value()["complete"], false);
    Ok(())
}

#[cfg(target_os = "macos")]
fn variable_typeface() -> Result<skia_safe::Typeface, String> {
    let bytes =
        std::fs::read("/System/Library/Fonts/SFNS.ttf").map_err(|error| error.to_string())?;
    skia_safe::FontMgr::default()
        .new_from_data(skia_safe::Data::new_copy(&bytes), Some(0))
        .ok_or_else(|| "variable capture typeface unavailable".into())
}

#[cfg(target_os = "macos")]
fn fill_instance_capture(base: &skia_safe::Typeface) -> Result<skia_safe::Typeface, String> {
    let mut ids = std::collections::HashSet::new();
    let mut first = None;
    for index in 0..super::capture::MAX_EVENTS_FOR_TEST {
        let weight = FIRST_CAPTURE_WEIGHT + index as f32;
        let face = variable_instance(base, weight)?;
        assert!(
            ids.insert(face.unique_id()),
            "variable face ID repeated at {weight}"
        );
        super::capture::record_instance(
            &face,
            &[resvg::usvg::FontVariation::new(WEIGHT_AXIS, weight)],
        );
        if first.is_none() {
            first = Some(face);
        }
    }
    first.ok_or_else(|| "first variable face missing".into())
}

#[cfg(target_os = "macos")]
fn variable_instance(
    base: &skia_safe::Typeface,
    weight: f32,
) -> Result<skia_safe::Typeface, String> {
    use skia_safe::font_arguments::{VariationPosition, variation_position::Coordinate};
    let coordinates = [Coordinate {
        axis: u32::from_be_bytes(WEIGHT_AXIS).into(),
        value: weight,
    }];
    let mut arguments = skia_safe::FontArguments::new();
    arguments.set_collection_index(0);
    let arguments = arguments.set_variation_design_position(VariationPosition {
        coordinates: &coordinates,
    });
    base.clone_with_arguments(&arguments)
        .ok_or_else(|| "variable face clone failed".into())
}
