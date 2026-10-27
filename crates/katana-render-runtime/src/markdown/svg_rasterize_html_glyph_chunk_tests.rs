use super::{SpanAlignment, position};
use tiny_skia::{Point, Transform};

const CSS_SCALE: f32 = 0.999_990_4;
const NOMINAL_SIZE: f32 = 14.0;
const PAINT_SIZE: f32 = 13.99;
const OBSERVED_ORIGINS: [f32; 3] = [-79.421_875, -37.437_5, -0.109_375];
const OBSERVED_BASELINE: f32 = 13.984_375;
const OBSERVED_ADVANCE: f32 = 9.331_207;
const NOMINAL_GLYPH_ADVANCE: f32 = 9.337_891;
const POSITION_TOLERANCE: f32 = 0.001;

#[test]
fn chunk_position_preserves_device_ratio_and_relative_glyph_advance() {
    let alignment = SpanAlignment {
        css_scale: CSS_SCALE,
        paint_css_size: PAINT_SIZE,
        source_origin: Point::from_xy(-79.478_03, NOMINAL_SIZE),
        paint_origin: Point::from_xy(OBSERVED_ORIGINS[0], OBSERVED_BASELINE),
        advance_ratio: PAINT_SIZE / (NOMINAL_SIZE * CSS_SCALE),
    };
    for device_ratio in [1.0, 2.0] {
        assert_device_position(alignment, device_ratio);
    }
}

fn assert_device_position(alignment: SpanAlignment, device_ratio: f32) {
    let scale = CSS_SCALE * device_ratio;
    let origin = alignment.source_origin;
    let first = Transform::from_translate(origin.x * scale, origin.y * scale);
    let next =
        Transform::from_translate((origin.x + NOMINAL_GLYPH_ADVANCE) * scale, origin.y * scale);
    let painted_first = position(alignment, first, scale);
    let painted_next = position(alignment, next, scale);
    assert_eq!(painted_first.tx / device_ratio, OBSERVED_ORIGINS[0]);
    assert_eq!(painted_first.ty / device_ratio, OBSERVED_BASELINE);
    let gap = (painted_next.tx - painted_first.tx) / device_ratio;
    assert!((gap - OBSERVED_ADVANCE).abs() < POSITION_TOLERANCE);
}

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph_chunk_source_tests.rs"]
mod source;
