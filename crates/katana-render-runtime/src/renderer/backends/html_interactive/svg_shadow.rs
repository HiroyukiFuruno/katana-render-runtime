use super::super::style::CssStyle;

const CSS_BLUR_TO_STANDARD_DEVIATION: f32 = 0.5;
const FILTER_STANDARD_DEVIATION_EXTENT: f32 = 3.0;
const SPREAD_CURVATURE_EXPONENT: i32 = 3;

pub(super) fn append_shadow(
    svg: &mut String,
    id: u64,
    geometry: (f32, f32, f32, f32),
    style: &CssStyle,
) {
    let (x, y, width, height) = geometry;
    let Some(shadow) = &style.box_shadow else {
        return;
    };
    let radius = style.resolved_border_radius(width, height);
    let expansion = shadow.spread_radius;
    let (width, height) = (width + expansion * 2.0, height + expansion * 2.0);
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let (x, y) = (
        x + shadow.offset_x - expansion,
        y + shadow.offset_y - expansion,
    );
    let deviation = shadow.blur_radius * CSS_BLUR_TO_STANDARD_DEVIATION;
    let filter = shadow_filter(svg, id, x, y, width, height, deviation);
    svg.push_str(&format!(
        r#"<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="{}" ry="{}" fill="{}"{filter}/>"#,
        spread_corner_radius(radius.0, expansion),
        spread_corner_radius(radius.1, expansion),
        super::escape_xml(&shadow.color),
    ));
}

fn shadow_filter(
    svg: &mut String,
    id: u64,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    deviation: f32,
) -> String {
    if deviation == 0.0 {
        return String::new();
    }
    let extent = deviation * FILTER_STANDARD_DEVIATION_EXTENT;
    svg.push_str(&format!(
        r#"<defs><filter id="krr-shadow-{id}" filterUnits="userSpaceOnUse" x="{}" y="{}" width="{}" height="{}" color-interpolation-filters="sRGB"><feGaussianBlur stdDeviation="{deviation}"/></filter></defs>"#,
        x - extent, y - extent, width + extent * 2.0, height + extent * 2.0,
    ));
    format!(r#" filter="url(#krr-shadow-{id})""#)
}

fn spread_corner_radius(radius: f32, spread: f32) -> f32 {
    if spread > radius {
        radius + spread * (1.0 + (radius / spread - 1.0).powi(SPREAD_CURVATURE_EXPONENT))
    } else {
        (radius + spread).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::spread_corner_radius;

    #[test]
    fn zero_blur_shadow_preserves_spread_without_a_filter() {
        let style = super::CssStyle::from_element(
            "div",
            &[(
                "style".to_string(),
                "box-shadow: 3px 4px 0px 2px #123456".to_string(),
            )],
            &super::CssStyle::browser_default(),
        );
        let mut svg = String::new();
        super::append_shadow(&mut svg, 1, (10.0, 20.0, 30.0, 40.0), &style);
        assert!(svg.contains(r#"x="11" y="22" width="34" height="44""#));
        assert!(svg.contains(r##"fill="#123456""##));
        assert!(!svg.contains("filter"));
    }

    #[test]
    fn spreading_a_square_shadow_keeps_square_corners() {
        assert_eq!(spread_corner_radius(0.0, 8.0), 0.0);
        assert_eq!(spread_corner_radius(10.0, 4.0), 14.0);
        assert_eq!(spread_corner_radius(4.0, -8.0), 0.0);
    }
}
