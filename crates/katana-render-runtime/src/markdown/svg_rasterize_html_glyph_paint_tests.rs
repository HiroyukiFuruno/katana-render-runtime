use super::super::PreparedSpan;
use super::super::XAlignment;
use super::super::uniform_raster_scale;
use super::{
    aligned_position_x, aligned_x, apply_run_origin_x_snap, is_identity_linear, scale_translation,
    text_font,
};
use skia_safe::{FontMgr, FontStyle, Point as SkiaPoint};
use tiny_skia::Transform;

const SOURCE_FONT_SIZE: f32 = 14.0;
const FRACTIONAL_RASTER_SCALE: f32 = 0.999_974_55;
const EXPECTED_EFFECTIVE_SIZE: f32 = 13.99;
const TEST_FILL: [u8; 4] = [0, 0, 0, u8::MAX];
const TEST_FONT_UNITS_PER_EM: f32 = 2_048.0;

#[test]
fn glyph_batching_rejects_non_identity_linear_transforms() {
    assert!(is_identity_linear(Transform::from_row(
        1.0, 0.0, 0.0, 1.0, 4.0, 8.0
    )));
    assert!(!is_identity_linear(Transform::from_row(
        1.0, 0.1, 0.0, 1.0, 4.0, 8.0
    )));
    assert!(!is_identity_linear(Transform::from_row(
        1.1, 0.0, 0.0, 1.0, 4.0, 8.0
    )));
    assert!(!is_identity_linear(Transform::from_row(
        1.0, 0.0, 0.0, -1.0, 4.0, 8.0
    )));
}

#[test]
fn uniform_scale_preserves_linear_parts_and_scales_baseline_translation() {
    let transform = Transform::from_row(1.0, 0.2, -0.1, 1.0, 3.0, 10.0);
    let scaled = scale_translation(transform, 2.0);

    assert_eq!(scaled.sx, transform.sx);
    assert_eq!(scaled.kx, transform.kx);
    assert_eq!(scaled.ky, transform.ky);
    assert_eq!(scaled.sy, transform.sy);
    assert_eq!(scaled.tx, 6.0);
    assert_eq!(scaled.ty, 20.0);
}

#[test]
fn uniform_scale_does_not_snap_near_identity_or_general_transforms() {
    assert_eq!(
        uniform_raster_scale(Transform::from_row(1.0001, 0.0, 0.0, 1.0001, 0.0, 0.0)),
        1.0001
    );
    assert_eq!(
        uniform_raster_scale(Transform::from_row(2.0, 0.0, 0.0, 3.0, 0.0, 0.0)),
        1.0
    );
    assert_eq!(
        uniform_raster_scale(Transform::from_row(2.0, 0.1, 0.0, 2.0, 0.0, 0.0)),
        1.0
    );
}

#[test]
fn paint_font_uses_effective_scaled_size_before_skia_font_creation() -> Result<(), String> {
    let typeface = FontMgr::default()
        .legacy_make_typeface(Some("Trebuchet MS"), FontStyle::normal())
        .ok_or("Trebuchet MS typeface unavailable")?;
    let span = trial_span(None);
    let fractional = text_font(&span, &typeface, FRACTIONAL_RASTER_SCALE);
    let identity = text_font(&span, &typeface, 1.0);

    assert_eq!(fractional.size(), EXPECTED_EFFECTIVE_SIZE);
    assert_eq!(identity.size(), SOURCE_FONT_SIZE);
    Ok(())
}

#[test]
fn aligned_x_preserves_identity_and_anchor() {
    assert_eq!(aligned_x(12.0, 18.0, 14.0, 1.0), 18.0);
    assert_eq!(aligned_x(12.0, 40.0, 14.0, 2.0), 40.0);
    let scaled_x = 20.0 * FRACTIONAL_RASTER_SCALE;
    let expected = 12.0 * FRACTIONAL_RASTER_SCALE
        + (scaled_x - 12.0 * FRACTIONAL_RASTER_SCALE)
            * (EXPECTED_EFFECTIVE_SIZE / (14.0 * FRACTIONAL_RASTER_SCALE));
    assert!((aligned_x(12.0, scaled_x, 14.0, FRACTIONAL_RASTER_SCALE) - expected).abs() < 0.0001);
}

#[test]
fn aligned_x_keeps_invalid_inputs_unchanged() {
    assert_eq!(aligned_x(12.0, 18.0, 0.0, 2.0), 18.0);
    assert_eq!(aligned_x(12.0, 18.0, f32::NAN, 2.0), 18.0);
}

#[test]
fn trial_keeps_nonzero_anchor_with_ratio_adjusted_width_at_each_dpr() {
    let css_scale = FRACTIONAL_RASTER_SCALE;
    let anchor_x = 9.0;
    let width = 31.130_86_f32;
    let layout = super::super::anchor::LayoutAnchor {
        width,
        anchor: resvg::usvg::TextAnchor::Middle,
        css_scale,
    };
    let alignment = XAlignment {
        anchor_x,
        layout: Some(layout),
        origin_snap_css_scale: None,
    };
    for dpr in [1.0, 2.0] {
        let raster_scale = css_scale * dpr;
        let nominal_css = SOURCE_FONT_SIZE * css_scale;
        let effective_css =
            super::super::super::SvgRasterizeOps::effective_html_font_size(nominal_css);
        let ratio = effective_css / nominal_css;
        let css_width = f64::from(width) * f64::from(css_scale) * f64::from(ratio);
        let snapped_width = (css_width * 64.0).ceil() / 64.0;
        let old_x = (anchor_x - width / 2.0) * raster_scale;
        let actual =
            aligned_position_x(&trial_span(Some(alignment)), alignment, old_x, raster_scale);
        let expected = anchor_x * raster_scale - snapped_width as f32 * dpr / 2.0;
        assert!((actual - expected).abs() < 0.00001);
    }
}

#[test]
fn run_origin_snap_preserves_relative_positions_at_dpr_one_and_two() {
    let css_scale = FRACTIONAL_RASTER_SCALE;
    let advance = 10.103_119;
    for dpr in [1.0, 2.0] {
        let raster_scale = css_scale * dpr;
        for css_origin in [10.02, -10.02] {
            let mut positions = [
                SkiaPoint::new(css_origin * dpr, 2.0),
                SkiaPoint::new((css_origin + advance) * dpr, 7.0),
            ];
            let original_gap = positions[1].x - positions[0].x;
            apply_run_origin_x_snap(&origin_snap_span(css_scale), &mut positions, raster_scale);
            let expected = if css_origin > 0.0 {
                10.015_625
            } else {
                -10.015_625
            };
            assert!((positions[0].x / dpr - expected).abs() < 0.000_001);
            assert!((positions[1].x - positions[0].x - original_gap).abs() < 0.000_001);
            assert_eq!(positions.map(|position| position.y), [2.0, 7.0]);
        }
    }
}

#[test]
fn run_origin_snap_leaves_unsupported_and_single_glyph_paths_unchanged() {
    let layout = super::super::anchor::LayoutAnchor {
        width: 7.0,
        anchor: resvg::usvg::TextAnchor::Middle,
        css_scale: 1.0,
    };
    let mut span = trial_span(Some(XAlignment {
        anchor_x: 0.0,
        layout: Some(layout),
        origin_snap_css_scale: Some(1.0),
    }));
    let mut positions = [SkiaPoint::new(-10.02, 3.0), SkiaPoint::new(1.25, 3.0)];
    let original = positions;
    apply_run_origin_x_snap(&span, &mut positions, 1.0);
    assert_eq!(positions, original);

    span.x_alignment = None;
    apply_run_origin_x_snap(&span, &mut positions, 1.0);
    assert_eq!(positions, original);
}

fn origin_snap_span(css_scale: f32) -> PreparedSpan {
    trial_span(Some(XAlignment {
        anchor_x: 0.0,
        layout: None,
        origin_snap_css_scale: Some(css_scale),
    }))
}

fn trial_span(alignment: Option<XAlignment>) -> PreparedSpan {
    PreparedSpan {
        fill: TEST_FILL,
        font: resvg::usvg::fontdb::ID::default(),
        glyphs: Vec::new(),
        glyph_ids: Vec::new(),
        upem: TEST_FONT_UNITS_PER_EM,
        font_size: SOURCE_FONT_SIZE,
        baseline_shift: 0.0,
        x_alignment: alignment,
    }
}
