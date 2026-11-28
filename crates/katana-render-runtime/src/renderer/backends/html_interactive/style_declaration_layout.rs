#[path = "style_declaration_border.rs"]
mod declaration_border;

use super::super::rotation::css_rotation_degrees;
use crate::renderer::backends::html_interactive::style::value::{css_line_height, css_number};
use crate::renderer::backends::html_interactive::style::{
    CssFloat, CssPosition, CssStyle, CssVerticalAlign, normalized_property_name,
};

impl CssStyle {
    pub(crate) fn apply_vertical_align(&mut self, value: &str, inherited: CssVerticalAlign) {
        self.vertical_align = match value.trim().to_ascii_lowercase().as_str() {
            "baseline" => CssVerticalAlign::Baseline,
            "top" => CssVerticalAlign::Top,
            "middle" => CssVerticalAlign::Middle,
            "bottom" => CssVerticalAlign::Bottom,
            "inherit" => inherited,
            "initial" | "unset" => CssVerticalAlign::Baseline,
            _ => self.vertical_align,
        };
    }

    pub(crate) fn apply(&mut self, name: &str, value: &str) {
        if self.apply_border_declaration(name, value) {
            return;
        }
        match normalized_property_name(name).as_ref() {
            "display" => self.apply_display(value),
            "color" => self.apply_color(value),
            "background" | "background-color" => self.apply_background(value),
            "box-shadow" => self.apply_box_shadow(value),
            "opacity" => self.apply_opacity(value),
            "border-collapse" => {
                self.border_collapse = value.trim().eq_ignore_ascii_case("collapse");
            }
            "position" => self.apply_position(value),
            "float" => self.apply_float(value),
            "appearance" | "-webkit-appearance" => self.apply_appearance(value),
            "transform" | "-webkit-transform" => self.apply_transform(value),
            "z-index" => self.apply_z_index(value),
            "list-style" | "list-style-type" => self.apply_list_style(value),
            _ => self.apply_layout_or_font(name, value),
        }
    }

    fn apply_list_style(&mut self, value: &str) {
        let mut recognized = false;
        for token in value.split_whitespace() {
            match token.to_ascii_lowercase().as_str() {
                "none" => {
                    self.list_style_none = true;
                    recognized = true;
                }
                "disc" | "circle" | "square" | "decimal" => {
                    self.list_style_none = false;
                    recognized = true;
                }
                _ => {}
            }
        }
        if !recognized && value.trim().eq_ignore_ascii_case("initial") {
            self.list_style_none = false;
        }
    }

    fn apply_opacity(&mut self, value: &str) {
        if let Ok(opacity) = value.trim().parse::<f32>()
            && opacity.is_finite()
        {
            self.opacity = opacity.clamp(0.0, 1.0);
        }
    }

    fn apply_z_index(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("auto") {
            self.z_index = None;
            return;
        }
        let Ok(z_index) = value.trim().parse::<i32>() else {
            return;
        };
        self.z_index = Some(z_index);
    }

    fn apply_position(&mut self, value: &str) {
        self.position = match value.trim().to_ascii_lowercase().as_str() {
            "static" => CssPosition::Static,
            "relative" => CssPosition::Relative,
            "absolute" => CssPosition::Absolute,
            "fixed" => CssPosition::Fixed,
            "sticky" => CssPosition::Sticky,
            _ => self.position,
        };
    }

    fn apply_float(&mut self, value: &str) {
        self.float = match value.trim().to_ascii_lowercase().as_str() {
            "none" => CssFloat::None,
            "left" => CssFloat::Left,
            "right" => CssFloat::Right,
            _ => self.float,
        };
    }

    fn apply_appearance(&mut self, value: &str) {
        self.appearance_none = match value.trim().to_ascii_lowercase().as_str() {
            "none" => true,
            "auto" | "initial" | "revert" => false,
            _ => self.appearance_none,
        };
    }

    fn apply_transform(&mut self, value: &str) {
        let value = value.trim();
        if value.eq_ignore_ascii_case("none") {
            self.rotation_degrees = 0.0;
            self.has_transform = false;
        } else if let Some(rotation) = css_rotation_degrees(value) {
            self.rotation_degrees = rotation;
            self.has_transform = true;
        }
    }

    fn apply_layout_or_font(&mut self, name: &str, value: &str) {
        if self.apply_box_property(name, value) || self.apply_typography_property(name, value) {
            return;
        }
        if self.apply_multicolumn_property(name, value) {
            return;
        }
        match name.to_ascii_lowercase().as_str() {
            "line-height" => self.apply_line_height(value),
            "gap" => self.apply_gap(value),
            "row-gap" => self.apply_row_gap(value),
            "flex-direction" => {
                self.flex_direction = value.parse().unwrap_or(self.flex_direction);
            }
            "flex-wrap" => self.flex_wrap = value.parse().unwrap_or(self.flex_wrap),
            "flex-grow" => self.flex_grow = css_number(value).unwrap_or(self.flex_grow),
            "flex-shrink" => self.flex_shrink = css_number(value).unwrap_or(self.flex_shrink),
            "flex-basis" => self.apply_flex_basis(value),
            "flex" => self.apply_flex(value),
            "align-items" => self.align_items = value.parse().ok().or(self.align_items),
            "justify-content" => {
                self.justify_content = value.parse().ok().or(self.justify_content);
            }
            _ => {}
        }
    }

    pub(crate) fn apply_line_height(&mut self, value: &str) {
        let Some((resolved, inherited_factor)) = css_line_height(
            value,
            self.font_size,
            self.viewport_width,
            self.viewport_height,
        ) else {
            return;
        };
        self.line_height = resolved;
        self.line_height_factor = inherited_factor;
        self.normal_line_height = value.trim().eq_ignore_ascii_case("normal");
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::rotation::css_rotation_degrees;
    use crate::renderer::backends::html_interactive::style::CssStyle;

    #[test]
    fn apply_invalid_transform_keeps_previous_rotation() {
        let mut style = CssStyle::browser_default();
        style.rotation_degrees = 15.0;

        style.apply("transform", "invalid");

        assert_eq!(style.rotation_degrees, 15.0);
    }

    #[test]
    fn apply_transform_parses_known_units() {
        let mut style = CssStyle::browser_default();

        style.apply("transform", "rotate(0.5turn)");
        assert!((style.rotation_degrees - 180.0).abs() < f32::EPSILON);

        style.apply("transform", "none");
        assert!((style.rotation_degrees - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn invalid_appearance_and_rotation_units_preserve_state() {
        let mut style = CssStyle::browser_default();
        style.appearance_none = true;

        style.apply("appearance", "unsupported");

        assert!(style.appearance_none);
        assert_eq!(css_rotation_degrees("rotate(10grad)"), None);
    }

    #[test]
    fn apply_invalid_line_height_preserves_previous_state() {
        let mut style = CssStyle::browser_default();
        let (original_line_height, original_line_height_factor) =
            (style.line_height, style.line_height_factor);

        style.apply("line-height", "invalid");

        assert_eq!(style.line_height, original_line_height);
        assert_eq!(style.line_height_factor, original_line_height_factor);
    }

    #[test]
    fn apply_opacity_with_finite_value_clamps_to_valid_range() {
        let mut style = CssStyle::browser_default();
        style.opacity = 0.5;

        style.apply("opacity", "2.0");

        assert_eq!(style.opacity, 1.0);

        style.apply("opacity", "NaN");
        assert_eq!(style.opacity, 1.0);

        style.apply("opacity", "invalid");
        assert_eq!(style.opacity, 1.0);
    }

    #[test]
    fn rotation_with_unsupported_unit_is_ignored() {
        let mut style = CssStyle::browser_default();
        style.rotation_degrees = 90.0;
        style.apply("transform", "rotate(10foo)");

        assert_eq!(style.rotation_degrees, 90.0);
    }

    #[test]
    fn css_rotation_unknown_unit_returns_none() {
        assert!(css_rotation_degrees("rotate(1foo)").is_none());
    }

    #[test]
    fn columns_shorthand_and_break_inside_are_parsed() {
        let inherited = CssStyle::browser_default();
        let style = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "columns: 2 280px; column-gap: 32px; break-inside: avoid".to_string(),
            )],
            &inherited,
        );

        assert_eq!(style.column_count, Some(2));
        assert_eq!(style.column_width, Some(280.0));
        assert_eq!(style.column_gap, 32.0);
        assert!(style.break_inside_avoid);
    }

    #[test]
    fn border_collapse_parses_as_a_table_layout_flag() {
        let inherited = CssStyle::browser_default();
        let collapsed = CssStyle::from_attributes(
            &[("style".to_string(), "border-collapse:collapse".to_string())],
            &inherited,
        );
        let separate = CssStyle::from_attributes(
            &[("style".to_string(), "border-collapse:separate".to_string())],
            &collapsed,
        );

        assert!(collapsed.border_collapse);
        assert!(!separate.border_collapse);
    }

    #[test]
    fn row_gap_normal_resets_only_row_gap() {
        let mut style = CssStyle::browser_default();
        style.row_gap = Some(12.0);

        style.apply("row-gap", "normal");

        assert_eq!(style.row_gap, Some(0.0));
    }
}
