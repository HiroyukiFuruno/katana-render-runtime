use skia_safe::{Font, M44, Point, SurfaceProps};

const MATRIX_COMPONENT_COUNT: usize = 16;

pub(super) fn surface_props_value(props: SurfaceProps) -> serde_json::Value {
    serde_json::json!({
        "flags": props.flags().bits(),
        "pixelGeometry": format!("{:?}", props.pixel_geometry()),
        "contrast": props.text_contrast(),
        "gamma": props.text_gamma(),
    })
}

pub(super) fn draw_event_value(
    font: &Font,
    glyph_ids: &[u16],
    positions: &[Point],
    origin: Point,
    local_to_device: M44,
    applied_coordinates: Option<&[serde_json::Value]>,
) -> serde_json::Value {
    let typeface = font.typeface();
    let mut matrix = [0.0; MATRIX_COMPONENT_COUNT];
    local_to_device.get_col_major(&mut matrix);
    serde_json::json!({
        "typefaceFamily": typeface.family_name(),
        "typefaceStyle": format!("{:?}", typeface.font_style()),
        "appliedVariationCoordinates": applied_coordinates,
        "font": font_value(font),
        "glyphIds": glyph_ids,
        "positions": position_values(positions),
        "origin": [origin.x, origin.y],
        "localToDeviceColumnMajor": matrix,
    })
}

fn font_value(font: &Font) -> serde_json::Value {
    serde_json::json!({
        "size": font.size(),
        "scaleX": font.scale_x(),
        "skewX": font.skew_x(),
        "subpixel": font.is_subpixel(),
        "linearMetrics": font.is_linear_metrics(),
        "baselineSnap": font.is_baseline_snap(),
        "embeddedBitmaps": font.is_embedded_bitmaps(),
        "embolden": font.is_embolden(),
        "forceAutoHinting": font.is_force_auto_hinting(),
        "hinting": format!("{:?}", font.hinting()),
        "edging": format!("{:?}", font.edging()),
    })
}

fn position_values(positions: &[Point]) -> Vec<[f32; 2]> {
    positions
        .iter()
        .map(|position| [position.x, position.y])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::super::capture_tests::{
        bundled_typeface, capture_fixture_draw, record_test_event,
    };
    use super::super::super::{PaintCaptureScope, capture};
    use skia_safe::{Font, M44, Point, SurfaceProps};

    #[test]
    fn paint_capture_records_actual_canvas_surface_properties() -> Result<(), String> {
        let (_, snapshot) = capture_fixture_draw(false)?;
        let actual = &snapshot.events.first().ok_or("draw missing")?["surfaceProps"];
        let props = SurfaceProps::default();
        assert_eq!(actual["flags"], serde_json::json!(props.flags().bits()));
        assert_eq!(
            actual["pixelGeometry"],
            format!("{:?}", props.pixel_geometry())
        );
        assert_eq!(actual["contrast"], serde_json::json!(props.text_contrast()));
        assert_eq!(actual["gamma"], serde_json::json!(props.text_gamma()));
        Ok(())
    }

    #[test]
    fn surface_properties_outside_capture_do_not_leak() -> Result<(), String> {
        let font = Font::new(bundled_typeface()?, 14.0);
        capture::record_surface_props(SurfaceProps::default());
        let scope = PaintCaptureScope::enter();
        record_test_event(&font, 0.0);
        assert!(scope.snapshot().events[0]["surfaceProps"].is_null());
        Ok(())
    }

    #[test]
    fn dropped_draw_consumes_pending_surface_properties() -> Result<(), String> {
        let font = Font::new(bundled_typeface()?, 14.0);
        let scope = PaintCaptureScope::enter();
        let glyphs = vec![1_u16; capture::MAX_GLYPHS_PER_EVENT_FOR_TEST + 1];
        capture::record_surface_props(SurfaceProps::default());
        capture::record_draw(&font, &glyphs, &[], Point::new(0.0, 0.0), M44::default());
        record_test_event(&font, 0.0);
        let snapshot = scope.snapshot();
        assert_eq!(snapshot.dropped, 1);
        assert_eq!(snapshot.events.len(), 1);
        assert!(snapshot.events[0]["surfaceProps"].is_null());
        Ok(())
    }
}
