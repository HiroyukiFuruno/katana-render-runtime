use super::super::super::{Policy, PolicyKind};
use super::super::{HtmlGlyphRasterizer, shift};
use crate::markdown::svg_rasterize::font_file_fixture;

const FONT_SIZE: f32 = 16.0;
const VALID_CSS_SCALE: f32 = 1.0;
const INVALID_CSS_SCALES: &[Option<f32>] =
    &[None, Some(f32::NAN), Some(0.0), Some(-VALID_CSS_SCALE)];

#[test]
fn middle_shift_rejects_invalid_scales_with_a_valid_file_face() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = font_file_fixture::create(
        directory.path().join("NotoSans-Regular.ttf"),
        font_file_fixture::FONT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    let rasterizer = HtmlGlyphRasterizer::new(&fixture.database);
    let metadata = rasterizer.face_metadata(fixture.face_id)?;
    let calculate = |scale| {
        shift(
            &rasterizer,
            fixture.face_id,
            &metadata,
            middle_policy(scale),
            None,
        )
    };
    for &scale in INVALID_CSS_SCALES {
        assert_eq!(calculate(scale)?, None);
    }
    assert!(calculate(Some(VALID_CSS_SCALE))?.is_some_and(f32::is_finite));
    Ok(())
}

fn middle_policy(css_scale: Option<f32>) -> Policy {
    Policy {
        kind: PolicyKind::Middle,
        font_size: FONT_SIZE,
        css_scale,
        primary_shift: None,
    }
}
