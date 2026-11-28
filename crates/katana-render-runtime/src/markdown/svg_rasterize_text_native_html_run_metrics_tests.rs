use super::super::test_support::{duplicate_width_face, regular_shape, roboto_flex, variable_face};
use super::{TextDxState, append_native_metrics};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use std::error::Error;

#[test]
fn native_metrics_append_real_variable_font_measurements() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = roboto_flex(&directory)?;
    let face = variable_face(&fixture);
    let mut state = TextDxState::new(1);
    let appended = SvgRasterizeOps::with_html_font_render_scope(|| {
        append_native_metrics(&mut state, &face, "A", regular_shape(), None)
    });
    assert_eq!(appended, Some(()));
    assert_eq!(state.finish(), None);
    Ok(())
}

#[test]
fn native_metrics_stop_when_real_variable_face_repeats_an_axis() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = roboto_flex(&directory)?;
    let face = duplicate_width_face(&fixture);
    let mut state = TextDxState::new(1);
    let appended = SvgRasterizeOps::with_html_font_render_scope(|| {
        append_native_metrics(&mut state, &face, "A", regular_shape(), None)
    });
    assert_eq!(appended, None);
    assert_eq!(state.finish(), None);
    Ok(())
}
