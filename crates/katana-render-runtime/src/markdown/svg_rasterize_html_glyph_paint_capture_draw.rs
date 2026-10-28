use skia_safe::{Font, M44, Point};

const MATRIX_COMPONENT_COUNT: usize = 16;

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
