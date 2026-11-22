use super::super::test_support::{regular_shape, roboto_flex, variable_face};
use super::{TextDxState, append_native_metrics};
use crate::markdown::svg_rasterize::SvgRasterizeOps;

#[test]
fn native_metrics_append_real_variable_font_measurements() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
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
