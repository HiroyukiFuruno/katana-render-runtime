use super::super::html_browser::HtmlBrowserViewport;
use super::style::CssStyle;

const HALF_PAINT_PIXEL: f32 = 0.5;

#[path = "svg_shadow.rs"]
mod shadow;

#[path = "svg_border.rs"]
mod border;
#[path = "svg_gradient.rs"]
mod gradient;

#[cfg(test)]
use gradient::LinearGradient;

pub(super) fn svg_header(viewport: HtmlBrowserViewport) -> String {
    let logical_width = viewport.logical_width();
    let logical_height = viewport.logical_height();
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xml:space="preserve" width="{}" height="{}" viewBox="0 0 {logical_width} {logical_height}"><rect width="100%" height="100%" fill="#ffffff"/>"##,
        viewport.width, viewport.height
    )
}

pub(super) fn box_svg(
    gradient_id: u64,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &CssStyle,
) -> String {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return String::new();
    }
    let (x, y, width, height) = snapped_box_geometry(x, y, width, height);
    let mut svg = String::new();
    shadow::append_shadow(&mut svg, gradient_id, (x, y, width, height), style);
    gradient::append_background(&mut svg, gradient_id, x, y, width, height, style);
    border::append_border(&mut svg, x, y, width, height, style);
    svg
}

/* WHY: Chrome は CSS box の両端を画素境界へ合わせて塗り、layout の小数座標は保持する。 */
fn snapped_box_geometry(x: f32, y: f32, width: f32, height: f32) -> (f32, f32, f32, f32) {
    let left = snap_paint_coordinate(x);
    let top = snap_paint_coordinate(y);
    let right = snap_paint_coordinate(x + width);
    let bottom = snap_paint_coordinate(y + height);
    (left, top, right - left, bottom - top)
}

pub(super) fn snap_paint_coordinate(value: f32) -> f32 {
    (value + HALF_PAINT_PIXEL).floor()
}

pub(super) fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::{LinearGradient, box_svg};
    use crate::renderer::backends::html_interactive::style::CssStyle;

    #[test]
    fn box_paint_snaps_both_edges_without_changing_layout_dimensions() {
        assert_eq!(
            super::snapped_box_geometry(10.375, 20.625, 100.25, 50.375),
            (10.0, 21.0, 101.0, 50.0)
        );
        assert_eq!(
            super::snapped_box_geometry(-0.5, -0.625, 2.0, 2.0),
            (0.0, -1.0, 2.0, 2.0)
        );
        let mut style = CssStyle::browser_default();
        style.background = Some("#1e293b".to_string());
        let svg = box_svg(0, 433.09375, 241.578_13, 802.90625, 2.0, &style);
        assert!(svg.contains(r#"x="433" y="242" width="803" height="2""#));
    }

    #[test]
    fn fractional_box_paint_matches_chrome_rows_at_multiple_device_scales()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut style = CssStyle::browser_default();
        style.background = Some("black".to_string());
        let painted = box_svg(0, 3.09375, 5.578125, 8.90625, 2.0, &style);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="white"/>{painted}</svg>"#
        );
        for scale in [1.0, 1.5, 2.0] {
            let image =
                crate::markdown::svg_rasterize::SvgRasterizeOps::rasterize_svg(&svg, scale)?;
            let x = (5.0 * scale) as u32;
            for y in 0..image.height {
                let start = ((y * image.width + x) * 4) as usize;
                let expected = if (6.0 * scale..8.0 * scale).contains(&(y as f32)) {
                    0
                } else {
                    255
                };
                assert_eq!(
                    &image.rgba[start..start + 3],
                    &[expected; 3],
                    "scale {scale}, row {y}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn linear_gradient_paints_angles_directions_and_distributed_stops() {
        let mut style = CssStyle::browser_default();
        style.background = Some(
            "linear-gradient(155deg, #173382 0%, rgba(44,74,198,0.8), #3952ff 100%)".to_string(),
        );
        let svg = box_svg(7, 0.0, 0.0, 200.0, 100.0, &style);

        assert!(svg.contains(r#"id="krr-gradient-7""#), "{svg}");
        assert!(svg.contains(r#"fill="url(#krr-gradient-7)""#), "{svg}");
        assert!(
            svg.contains(r##"offset="0%" stop-color="#173382""##),
            "{svg}"
        );
        assert!(
            svg.contains(r#"offset="50%" stop-color="rgba(44,74,198,0.8)""#),
            "{svg}"
        );
        assert!(
            svg.contains(r##"offset="100%" stop-color="#3952ff""##),
            "{svg}"
        );

        assert_eq!(
            LinearGradient::parse("linear-gradient(to right, red, blue)")
                .map(|gradient| gradient.angle_degrees),
            Some(90.0)
        );
        assert!(LinearGradient::parse("linear-gradient(red)").is_none());
    }

    #[test]
    fn unsupported_background_images_do_not_emit_invalid_svg_fill() {
        let mut style = CssStyle::browser_default();
        style.background = Some("url(background.png)".to_string());
        assert_eq!(box_svg(0, 0.0, 0.0, 10.0, 10.0, &style), "");
    }

    #[test]
    fn box_shadow_blurs_the_original_alpha_behind_the_background() {
        let attributes = [(
            "style".to_string(),
            "box-shadow: 0 10px 28px rgba(15,40,89,0.14)".to_string(),
        )];
        let mut style = CssStyle::from_element("div", &attributes, &CssStyle::browser_default());
        style.background = Some("#ffffff".to_string());
        let svg = box_svg(0, 10.0, 20.0, 100.0, 50.0, &style);

        assert!(
            svg.contains(r#"<feGaussianBlur stdDeviation="14"/>"#),
            "{svg}"
        );
        assert!(
            svg.contains(r#"color-interpolation-filters="sRGB""#),
            "{svg}"
        );
        assert!(!svg.contains("fill-opacity"), "{svg}");
        let paint_order = (
            svg.find("url(#krr-shadow-0)"),
            svg.rfind(r##"fill="#ffffff""##),
        );
        assert!(
            matches!(paint_order, (Some(shadow), Some(background)) if shadow < background),
            "{svg}"
        );
    }

    #[test]
    fn non_positive_or_non_finite_boxes_do_not_emit_invalid_svg_geometry() {
        let mut style = CssStyle::browser_default();
        style.background = Some("red".to_string());

        assert_eq!(box_svg(0, 0.0, 0.0, 0.0, 10.0, &style), "");
        assert_eq!(box_svg(0, 0.0, 0.0, 10.0, -1.0, &style), "");
        assert_eq!(box_svg(0, 0.0, 0.0, f32::NAN, 10.0, &style), "");
    }

    #[test]
    fn shadow_layers_with_non_positive_geometry_are_not_painted() {
        let attributes = [(
            "style".to_string(),
            "box-shadow: 0 0 0 -20px red".to_string(),
        )];
        let mut style = CssStyle::from_element("div", &attributes, &CssStyle::browser_default());
        style.background = Some("white".to_string());

        let svg = box_svg(0, 0.0, 0.0, 10.0, 10.0, &style);

        assert!(!svg.contains("fill-opacity"), "{svg}");
        assert!(svg.contains("fill=\"white\""), "{svg}");
    }
}
