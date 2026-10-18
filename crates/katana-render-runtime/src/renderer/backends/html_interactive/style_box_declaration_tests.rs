#[cfg(test)]
mod tests {
    use crate::renderer::backends::html_interactive::style::{
        CssBoxSizing, CssLength, CssOverflow, CssOverflowAxis, CssStyle,
    };

    const PADDING_TOP_DEFAULT: f32 = 1.0;
    const PADDING_RIGHT_DEFAULT: f32 = 2.0;
    const PADDING_BOTTOM_DEFAULT: f32 = 3.0;
    const PADDING_LEFT_DEFAULT: f32 = 4.0;
    const MARGIN_DEFAULT_SIDE_VALUE: f32 = 0.0;
    const HEIGHT_REM_VALUE: f32 = 112.0;
    const MAX_WIDTH: f32 = 60.0;
    const MIN_HEIGHT_EM_VALUE: f32 = 16.0;
    const BORDER_WIDTH_INVALID_COMPONENT: f32 = 3.0;
    const BORDER_LEFT_WIDTH: f32 = 7.0;

    #[test]
    fn apply_box_property_displays_invalid_values_are_ignored() {
        let mut style = CssStyle::browser_default();

        assert!(!style.apply_box_property("unsupported-property", "10px"));
        assert_eq!(style.box_sizing, CssBoxSizing::ContentBox);
        assert_eq!(style.overflow, CssOverflow::Visible);
    }

    #[test]
    fn apply_box_sizing_resolves_known_values_and_keeps_unknown() {
        let mut style = CssStyle::browser_default();

        style.apply_box_property("box-sizing", "border-box");
        assert_eq!(style.box_sizing, CssBoxSizing::BorderBox);

        style.apply_box_property("box-sizing", "unsupported");
        assert_eq!(style.box_sizing, CssBoxSizing::BorderBox);
    }

    #[test]
    fn apply_overflow_sets_clip_for_hidden_like_values_and_keeps_visible() {
        let mut style = CssStyle::browser_default();

        style.apply_box_property("overflow", "hidden");
        assert_eq!(style.overflow, CssOverflow::Clip);

        style.apply_box_property("overflow", "visible");
        assert_eq!(style.overflow, CssOverflow::Visible);

        style.apply_box_property("overflow", "unsupported");
        assert_eq!(style.overflow, CssOverflow::Visible);
    }

    #[test]
    fn overflow_shorthand_preserves_specified_axes_and_resolves_visible_axis() {
        let mut style = CssStyle::browser_default();

        style.apply_box_property("overflow", "ScRoLl ClIp");

        assert_eq!(
            style.specified_overflow_axes(),
            (CssOverflowAxis::Scroll, CssOverflowAxis::Clip)
        );
        assert_eq!(
            style.computed_overflow_axes(),
            (CssOverflowAxis::Scroll, CssOverflowAxis::Hidden)
        );

        style.apply_box_property("overflow", "hidden");
        assert_eq!(
            style.specified_overflow_axes(),
            (CssOverflowAxis::Hidden, CssOverflowAxis::Hidden)
        );
    }

    #[test]
    fn overflow_axis_longhands_are_independent_and_apply_computed_rules() {
        let mut style = CssStyle::browser_default();

        style.apply_box_property("overflow-x", "clip");
        style.apply_box_property("overflow-y", "auto");

        assert_eq!(
            style.computed_overflow_axes(),
            (CssOverflowAxis::Hidden, CssOverflowAxis::Auto)
        );
        assert!(style.clips_overflow());
    }

    #[test]
    fn invalid_overflow_values_leave_specified_axes_unchanged() {
        let mut style = CssStyle::browser_default();
        style.apply_box_property("overflow", "auto hidden");
        let before = style.specified_overflow_axes();

        style.apply_box_property("overflow", "auto hidden clip");
        style.apply_box_property("overflow-x", "invalid");

        assert_eq!(style.specified_overflow_axes(), before);
    }

    #[test]
    fn legacy_clip_projection_keeps_direct_internal_assignments_compatible() {
        let mut style = CssStyle::browser_default();
        style.overflow = CssOverflow::Clip;

        assert_eq!(
            style.computed_overflow_axes(),
            (CssOverflowAxis::Clip, CssOverflowAxis::Clip)
        );
        assert!(style.clips_overflow());
    }

    #[test]
    fn apply_padding_property_keeps_each_edge_with_shorthand_and_invalid_value() {
        let mut style = CssStyle::browser_default();
        style.padding_top = PADDING_TOP_DEFAULT;
        style.padding_right = PADDING_RIGHT_DEFAULT;
        style.padding_bottom = PADDING_BOTTOM_DEFAULT;
        style.padding_left = PADDING_LEFT_DEFAULT;
        style.apply_padding_property("padding", "8px 9px");
        assert_eq!(
            [
                style.padding_top,
                style.padding_right,
                style.padding_bottom,
                style.padding_left
            ],
            [8.0, 9.0, 8.0, 9.0]
        );

        style.apply_padding_property("padding", "1px 2px 3px 4px 5px");
        assert_eq!(
            [
                style.padding_top,
                style.padding_right,
                style.padding_bottom,
                style.padding_left
            ],
            [8.0, 9.0, 8.0, 9.0]
        );
    }

    #[test]
    fn apply_padding_property_drops_unknown_name_unmodified() {
        let mut style = CssStyle::browser_default();
        style.padding_left = PADDING_LEFT_DEFAULT;
        style.apply_padding_property("padding-inline", "4px");
        assert_eq!(style.padding_left, PADDING_LEFT_DEFAULT);
    }

    #[test]
    fn apply_margin_property_drops_unknown_name_unmodified() {
        let mut style = CssStyle::browser_default();
        style.margin_top = 9.0;
        style.apply_margin_property("margin-inline", "4px");
        assert_eq!(style.margin_top, 9.0);
    }

    #[test]
    fn apply_margin_property_sets_each_edge_and_ignores_invalid_shorthand() {
        let mut style = CssStyle::browser_default();
        style.apply_margin_property("margin", "1px 2px 3px 4px");
        assert_eq!(
            [
                style.margin_top,
                style.margin_right,
                style.margin_bottom,
                style.margin_left
            ],
            [1.0, 2.0, 3.0, 4.0]
        );

        style.apply_margin_property("margin", "1px 2px 3px 4px 5px");
        assert_eq!(
            [
                style.margin_top,
                style.margin_right,
                style.margin_bottom,
                style.margin_left
            ],
            [1.0, 2.0, 3.0, 4.0]
        );
    }

    #[test]
    fn auto_horizontal_margins_are_preserved_and_reset_by_lengths() {
        let mut style = CssStyle::browser_default();
        style.apply_margin_property("margin", "0 auto");

        assert_eq!(
            [style.margin_left, style.margin_right],
            [MARGIN_DEFAULT_SIDE_VALUE; 2]
        );
        assert!(style.margin_left_auto);
        assert!(style.margin_right_auto);

        style.apply_margin_property("margin-left", "12px");
        assert_eq!(style.margin_left, 12.0);
        assert!(!style.margin_left_auto);
        assert!(style.margin_right_auto);
    }

    #[test]
    fn apply_dimension_converts_known_longhands_and_preserves_unknown() {
        let mut style = CssStyle::browser_default();
        style.apply_dimensions("width", "120%");
        assert_eq!(style.width, Some(CssLength::Percent(1.2)));
        style.apply_dimensions("max-width", "60px");
        assert_eq!(style.max_width, Some(CssLength::Px(MAX_WIDTH)));
        style.apply_dimensions("height", "7rem");
        assert_eq!(style.height, Some(HEIGHT_REM_VALUE));
        style.apply_dimensions("min-height", "1em");
        assert_eq!(style.min_height, MIN_HEIGHT_EM_VALUE);

        style.apply_dimensions("line-height", "42px");
        assert_eq!(style.min_height, MIN_HEIGHT_EM_VALUE);
    }

    #[test]
    fn border_width_shorthand_ignores_invalid_component_count() {
        let mut style = CssStyle::browser_default();
        style.border_width = BORDER_WIDTH_INVALID_COMPONENT;

        style.apply_box_property("border-width", "1px 2px 3px 4px 5px");

        assert_eq!(style.border_width, BORDER_WIDTH_INVALID_COMPONENT);
    }

    #[test]
    fn border_side_width_accepts_lengths_and_ignores_invalid_values() {
        let mut style = CssStyle::browser_default();
        style.apply_box_property("border-left-width", "7px");
        assert_eq!(style.border_left_width, Some(BORDER_LEFT_WIDTH));

        style.apply_box_property("border-left-width", "invalid");
        assert_eq!(style.border_left_width, Some(BORDER_LEFT_WIDTH));
    }
}
