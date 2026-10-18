use super::super::constants::{CONTROL_HEIGHT, DEFAULT_INPUT_SIZE, INPUT_HEIGHT, MIN_LAYOUT_WIDTH};
use super::super::document::attribute;
use super::super::style::CssStyle;
use super::super::text_metrics::text_width;
use crate::markdown::svg_rasterize::SvgRasterizeOps;

const CHECKBOX_INSET_SCALE: f32 = 4.0;
const CHECKBOX_CHECKMARK_LEFT_RATIO: f32 = 0.48;
const CHECKBOX_CHECKMARK_MIDDLE_X_RATIO: f32 = 0.08;
const CHECKBOX_CHECKMARK_MIDDLE_Y_RATIO: f32 = 0.36;
const CHECKBOX_CHECKMARK_RIGHT_RATIO: f32 = 0.55;
const CHECKBOX_CHECKMARK_TOP_RATIO: f32 = 0.4;
const CHECKBOX_MIN_INSET: f32 = 2.0;

pub(super) const CHECKBOX_STROKE: &str = "#8c959f";
pub(super) const CHECKBOX_CHECK: &str = "#16a34a";

#[derive(Clone, Copy)]
pub(super) struct InputGeometry {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
}

pub(super) fn input_geometry(
    x: f32,
    y: f32,
    width: f32,
    attributes: &[(String, String)],
    style: &CssStyle,
) -> InputGeometry {
    let available_width = available_input_width(width, style);
    let width = input_width(attributes, available_width, style);
    InputGeometry {
        x: x + style.margin_left,
        y: y + style.margin_top,
        width,
        height: input_height(style),
    }
}

fn available_input_width(width: f32, style: &CssStyle) -> f32 {
    (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH)
}

fn input_width(attributes: &[(String, String)], available: f32, style: &CssStyle) -> f32 {
    style
        .explicit_width(available)
        .unwrap_or_else(|| intrinsic_input_width(attributes, style))
}

fn intrinsic_input_width(attributes: &[(String, String)], style: &CssStyle) -> f32 {
    let size = attribute(attributes, "size")
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|size| *size > 0)
        .unwrap_or(DEFAULT_INPUT_SIZE);
    (character_width_for_size(size, style)
        + style.padding_left
        + style.padding_right
        + style.border_left_width()
        + style.border_right_width())
    .max(MIN_LAYOUT_WIDTH)
}

fn input_height(style: &CssStyle) -> f32 {
    style
        .height
        .map(|height| style.outer_height(height))
        .unwrap_or_else(|| style.outer_height(style.line_height).max(INPUT_HEIGHT))
        .max(style.minimum_outer_height())
}

fn character_width_for_size(size: usize, style: &CssStyle) -> f32 {
    let representative =
        text_width("0", style).max(style.font_size * style.text_character_width_factor());
    let measured = SvgRasterizeOps::html_input_character_metrics(
        &style.font_family,
        style.font_size,
        style.font_weight,
        style.italic,
    )
    .map(|metrics| {
        if metrics.average_from_primary_font {
            let average = metrics.average.round();
            (average * size as f32).ceil() + metrics.maximum.round() - average
        } else {
            (metrics.average * size as f32).ceil()
        }
    });
    measured.unwrap_or_else(|| (representative * size as f32).ceil())
}

pub(super) fn checkbox_geometry(x: f32, y: f32, width: f32, style: &CssStyle) -> InputGeometry {
    let available_width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
    let width = style
        .explicit_width(available_width)
        .unwrap_or(CONTROL_HEIGHT)
        .min(available_width);
    InputGeometry {
        x: x + style.margin_left,
        y: y + style.margin_top,
        width,
        height: style.height.unwrap_or(width).max(style.min_height),
    }
}

pub(super) fn checkbox_inset(dimension: f32) -> f32 {
    CHECKBOX_MIN_INSET.min(dimension / CHECKBOX_INSET_SCALE)
}

pub(super) fn is_checkbox(attributes: &[(String, String)]) -> bool {
    attribute(attributes, "type").is_some_and(|value| value.eq_ignore_ascii_case("checkbox"))
}

pub(super) const fn checkbox_check_ratio_left() -> f32 {
    CHECKBOX_CHECKMARK_LEFT_RATIO
}

pub(super) const fn checkbox_check_ratio_middle_x() -> f32 {
    CHECKBOX_CHECKMARK_MIDDLE_X_RATIO
}

pub(super) const fn checkbox_check_ratio_middle_y() -> f32 {
    CHECKBOX_CHECKMARK_MIDDLE_Y_RATIO
}

pub(super) const fn checkbox_check_ratio_right() -> f32 {
    CHECKBOX_CHECKMARK_RIGHT_RATIO
}

pub(super) const fn checkbox_check_ratio_top() -> f32 {
    CHECKBOX_CHECKMARK_TOP_RATIO
}

#[cfg(test)]
mod tests {
    use super::{INPUT_HEIGHT, input_geometry};
    use crate::renderer::backends::html_interactive::control_style::ua_control_style;
    use crate::renderer::backends::html_interactive::style::CssStyle;

    #[test]
    fn input_size_attribute_scales_intrinsic_width() {
        let style = CssStyle::browser_default();
        let small = input_geometry(
            0.0,
            0.0,
            500.0,
            &[("size".to_string(), "10".to_string())],
            &style,
        );
        let large = input_geometry(
            0.0,
            0.0,
            500.0,
            &[("size".to_string(), "20".to_string())],
            &style,
        );
        assert!(small.width < large.width);
        assert!(large.height >= INPUT_HEIGHT);
    }

    #[test]
    fn author_width_and_height_override_intrinsic_control_size() {
        let style = CssStyle::from_element(
            "input",
            &[(
                "style".to_string(),
                "width: 240px; height: 40px".to_string(),
            )],
            &CssStyle::browser_default(),
        );
        let geometry = input_geometry(0.0, 0.0, 500.0, &[], &style);
        assert_eq!(geometry.width, 240.0);
        assert_eq!(geometry.height, 40.0);
    }

    #[test]
    fn intrinsic_input_width_includes_resolved_padding_and_border() {
        let inherited = CssStyle::browser_default();
        let plain = CssStyle::from_element("input", &[], &inherited);
        let authored = CssStyle::from_element(
            "input",
            &[(
                "style".to_string(),
                "padding:10px;border:3px solid".to_string(),
            )],
            &inherited,
        );
        let plain_width = input_geometry(0.0, 0.0, 500.0, &[], &plain).width;
        let authored_width = input_geometry(0.0, 0.0, 500.0, &[], &authored).width;
        assert!(authored_width > plain_width);
    }

    #[test]
    fn intrinsic_input_width_uses_the_resolved_font_metrics_when_available() {
        let mut style = ua_control_style(&CssStyle::browser_default());
        style.font_family = "Arial".to_string();

        let geometry = input_geometry(
            0.0,
            0.0,
            500.0,
            &[("size".to_string(), "20".to_string())],
            &style,
        );

        assert!(geometry.width > 0.0);
        assert!(geometry.width.is_finite());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_input_default_width_uses_blink_character_metrics() {
        let style = ua_control_style(&CssStyle::browser_default());
        let geometry = input_geometry(0.0, 0.0, 500.0, &[], &style);
        assert!(
            (geometry.width - 145.0).abs() < 0.01,
            "width {}",
            geometry.width
        );
    }
}
