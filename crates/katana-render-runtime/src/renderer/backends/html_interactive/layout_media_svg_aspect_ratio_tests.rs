use super::{TestResult, assert_geometry, capture_html, capture_pixels};

#[test]
fn svg_preserve_aspect_ratio_controls_paint_in_the_same_css_box() -> TestResult {
    const BOX_SIZE: f32 = 80.0;
    let html = |aspect| {
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" style="width:80px;height:80px" viewBox="0 0 80 40" {aspect}><rect width="80" height="40" fill="#ff0000"/></svg>"##
        );
        capture_html(&svg, false)
    };
    let meet = html("");
    let stretch = html(r#"preserveAspectRatio="none""#);
    assert_geometry(&meet, BOX_SIZE, BOX_SIZE)?;
    assert_geometry(&stretch, BOX_SIZE, BOX_SIZE)?;
    let meet_pixels = red_pixel_count(&meet)?;
    let stretch_pixels = red_pixel_count(&stretch)?;
    assert!(meet_pixels > 0, "default aspect ratio lost visible SVG");
    assert!(
        stretch_pixels > meet_pixels,
        "expected more red pixels after stretching"
    );
    Ok(())
}

pub(super) fn red_pixel_count(html: &str) -> TestResult<usize> {
    const RGBA_CHANNELS: usize = 4;
    const OPAQUE_RED: [u8; RGBA_CHANNELS] = [u8::MAX, 0, 0, u8::MAX];
    Ok(capture_pixels(html)?
        .chunks_exact(RGBA_CHANNELS)
        .filter(|pixel| *pixel == OPAQUE_RED)
        .count())
}
