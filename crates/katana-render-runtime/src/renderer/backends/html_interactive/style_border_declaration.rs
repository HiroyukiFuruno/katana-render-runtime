#[path = "style_border_properties.rs"]
mod border_properties;
#[path = "style_border_side.rs"]
mod border_side;
#[path = "style_border_values.rs"]
mod border_values;

use super::CssStyle;

#[cfg(test)]
mod tests {
    use super::CssStyle;

    #[test]
    fn border_color_resets_edge_colors_and_ignores_invalid_values() {
        let mut style = CssStyle::browser_default();
        style.border_top_color = Some("red".to_string());

        style.apply_border_color("");
        assert_eq!(style.border_top_color.as_deref(), Some("red"));

        style.apply_border_color("blue");
        assert_eq!(style.border.as_deref(), Some("blue"));
        assert!(style.border_top_color.is_none());
    }

    #[test]
    fn border_side_handles_none_invalid_names_and_invalid_colors() {
        let mut style = CssStyle::browser_default();
        style.apply_border_side("color", "red");
        style.apply_border_side("border-top", "none");
        assert_eq!(style.border_top_width, Some(0.0));

        style.apply_border_side_color("border-top-color", "");
        assert!(style.border_top_color.is_none());

        style.apply_border_side_color("color", "red");
        assert!(style.border_top_color.is_none());
    }

    #[test]
    fn border_side_without_width_uses_color_default_or_current_width() {
        let mut style = CssStyle::browser_default();
        style.apply_border_side("border-right", "red");
        assert_eq!(style.border_right_width, Some(1.0));

        style.border_width = 4.0;
        style.apply_border_side("border-bottom", "solid");
        assert_eq!(style.border_bottom_width, Some(4.0));
    }

    #[test]
    fn unknown_border_edges_preserve_style() {
        let mut style = CssStyle::browser_default();
        style.border_width = 3.0;

        assert_eq!(style.border_side_width("diagonal"), 3.0);
        style.set_border_side_width("diagonal", 7.0);
        style.set_border_side_color("diagonal", "red".to_string());
        assert_eq!(style.border_width, 3.0);
    }

    #[test]
    fn border_side_color_sets_valid_edges() {
        let mut style = CssStyle::browser_default();

        style.apply_border_side_color("border-left-color", "blue");

        assert_eq!(style.border_left_color.as_deref(), Some("blue"));
    }

    #[test]
    fn border_style_shorthand_expands_css_four_side_values() {
        let mut style = CssStyle::browser_default();
        style.apply_border_style("dashed dotted none solid");

        assert_eq!(
            style.border_top_style,
            Some(super::super::types::CssBorderStyle::Dashed)
        );
        assert_eq!(
            style.border_right_style,
            Some(super::super::types::CssBorderStyle::Dotted)
        );
        assert_eq!(
            style.border_bottom_style,
            Some(super::super::types::CssBorderStyle::None)
        );
        assert_eq!(
            style.border_left_style,
            Some(super::super::types::CssBorderStyle::Solid)
        );
        assert_eq!(style.border_bottom_width, None);
    }

    #[test]
    fn border_width_without_style_keeps_width_but_has_zero_used_width() {
        let mut style = CssStyle::browser_default();
        style.apply_border("4px red");

        assert_eq!(style.border_width, 4.0);
        assert_eq!(
            style.border_style,
            super::super::types::CssBorderStyle::None
        );
        assert_eq!(style.border_top_width(), 0.0);
    }

    #[test]
    fn border_style_none_preserves_specified_width_for_later_style() {
        let mut style = CssStyle::browser_default();
        style.border_width = 4.0;

        style.apply_border_style("none");
        assert_eq!(style.border_width, 4.0);
        assert_eq!(
            style.border_style,
            super::super::types::CssBorderStyle::None
        );

        style.apply_border_style("solid");
        assert_eq!(style.border_width, 4.0);
    }

    #[test]
    fn border_color_without_width_keeps_css_none_style_and_zero_used_width() {
        let mut style = CssStyle::browser_default();
        style.apply_border_color("red");

        assert_eq!(
            style.border_style,
            super::super::types::CssBorderStyle::None
        );
        assert_eq!(style.border_top_width(), 0.0);
        assert_eq!(style.border_right_width(), 0.0);
        assert_eq!(style.border_bottom_width(), 0.0);
        assert_eq!(style.border_left_width(), 0.0);
    }

    #[test]
    fn border_side_shorthand_preserves_style_and_color() {
        let mut style = CssStyle::browser_default();
        style.apply_border_side("border-bottom", "1px dashed #334455");

        assert_eq!(
            style.border_bottom_style,
            Some(super::super::types::CssBorderStyle::Dashed)
        );
        assert_eq!(style.border_bottom_width, Some(1.0));
        assert_eq!(style.border_bottom_color.as_deref(), Some("#334455"));
    }
}
