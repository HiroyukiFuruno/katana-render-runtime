#[cfg(test)]
mod tests {
    use crate::renderer::backends::html_interactive::style::{
        CssStyle, CssTextAlign, CssTextSpacingTrim, CssTextTransform, CssWhiteSpace,
    };

    const BASE_FONT_SIZE: f32 = 14.0;
    const BASE_LINE_HEIGHT: f32 = 19.6;
    const FONT_WEIGHT_THICK: u16 = 700;
    const FONT_WEIGHT_BOLDER: u16 = 600;
    const FONT_WEIGHT_NORMAL: u16 = 400;
    const FONT_WEIGHT_LIGHTER: u16 = 400;
    const FONT_SPACING_NEGATIVE_HALF: f32 = -0.5;
    const FONT_WEIGHT_THIN: u16 = 300;
    const EQUALITY_EPSILON: f32 = 0.01;

    #[test]
    fn typography_dispatches_known_properties_and_rejects_unknown() {
        let mut style = CssStyle::browser_default();

        assert!(style.apply_typography_property("font-size", "2rem"));
        assert!(style.apply_typography_property("font-weight", "700"));
        assert_eq!(style.font_weight, FONT_WEIGHT_THICK);
        assert!(style.apply_typography_property("font-style", "italic"));
        assert!(style.italic);

        assert!(!style.apply_typography_property("line-height", "160%"));
    }

    #[test]
    fn font_shorthand_sets_size_line_height_family_style_and_weight() {
        let mut style = CssStyle::browser_default();

        style.apply_typography_property(
            "font",
            "italic 300 14px/1.4 'Helvetica Neue', Arial, sans-serif",
        );

        assert_eq!(style.font_size, BASE_FONT_SIZE);
        assert!((style.line_height - BASE_LINE_HEIGHT).abs() < EQUALITY_EPSILON);
        assert_eq!(style.font_family, "'Helvetica Neue', Arial, sans-serif");
        assert_eq!(style.font_weight, FONT_WEIGHT_THIN);
        assert!(style.italic);
    }

    #[test]
    fn invalid_font_shorthand_preserves_the_current_font() {
        let mut style = CssStyle::browser_default();
        let original = (
            style.font_size,
            style.line_height,
            style.font_family.clone(),
            style.font_weight,
        );

        style.apply_typography_property("font", "italic invalid");

        assert_eq!(
            (
                style.font_size,
                style.line_height,
                style.font_family,
                style.font_weight,
            ),
            original
        );
    }

    #[test]
    fn numeric_and_relative_font_weights_preserve_css_weight_values() {
        let mut style = CssStyle::browser_default();
        style.apply_typography_property("font-weight", "600");
        assert_eq!(style.font_weight, FONT_WEIGHT_BOLDER);
        style.apply_typography_property("font-weight", "lighter");
        assert_eq!(style.font_weight, FONT_WEIGHT_LIGHTER);
        style.apply_typography_property("font-weight", "bolder");
        assert_eq!(style.font_weight, FONT_WEIGHT_THICK);
        style.apply_typography_property("font-weight", "normal");
        assert_eq!(style.font_weight, FONT_WEIGHT_NORMAL);
    }

    #[test]
    fn font_family_keeps_current_if_empty() {
        let mut style = CssStyle::browser_default();
        style.font_family = "Noto Sans, sans-serif".to_string();

        style.apply_typography_property("font-family", "  ");

        assert_eq!(style.font_family, "Noto Sans, sans-serif");
    }

    #[test]
    fn font_feature_settings_are_parsed_reset_and_inherited() {
        let mut style = CssStyle::browser_default();
        style.apply_typography_property("font-feature-settings", r#""palt" 1"#);
        assert_eq!(style.font_feature_settings.as_deref(), Some(r#""palt" 1"#));

        let inherited = CssStyle::from_attributes(&[], &style);
        assert_eq!(inherited.font_feature_settings, style.font_feature_settings);

        style.apply_typography_property("font-feature-settings", "normal");
        assert_eq!(style.font_feature_settings, None);
        style.apply_typography_property("font-feature-settings", "  ");
        assert_eq!(style.font_feature_settings, None);
    }

    #[test]
    fn text_spacing_trim_is_typed_and_inherited() {
        let mut style = CssStyle::browser_default();
        assert_eq!(style.text_spacing_trim, CssTextSpacingTrim::Normal);
        style.apply_typography_property("text-spacing-trim", "space-all");
        assert_eq!(style.text_spacing_trim, CssTextSpacingTrim::SpaceAll);
        let inherited = style.inherited_text_style();
        assert_eq!(inherited.text_spacing_trim, CssTextSpacingTrim::SpaceAll);
        style.apply_typography_property("text-spacing-trim", "normal");
        assert_eq!(style.text_spacing_trim, CssTextSpacingTrim::Normal);
        style.apply_typography_property("text-spacing-trim", "space-all");
        style.apply_typography_property("text-spacing-trim", "inherit");
        assert_eq!(style.text_spacing_trim, CssTextSpacingTrim::SpaceAll);
        let child = CssStyle::from_attributes(
            &[("style".to_string(), "text-spacing-trim: unset".to_string())],
            &style,
        );
        assert_eq!(child.text_spacing_trim, CssTextSpacingTrim::SpaceAll);
        style.apply_typography_property("text-spacing-trim", "initial");
        assert_eq!(style.text_spacing_trim, CssTextSpacingTrim::Normal);
    }

    #[test]
    fn white_space_nowrap_is_parsed_and_inherited() {
        let mut style = CssStyle::browser_default();
        style.apply_typography_property("white-space", "nowrap");
        assert_eq!(style.white_space, CssWhiteSpace::NoWrap);

        let inherited = CssStyle::from_attributes(&[], &style);
        assert_eq!(inherited.white_space, CssWhiteSpace::NoWrap);

        style.apply_typography_property("white-space", "normal");
        assert_eq!(style.white_space, CssWhiteSpace::Normal);
        style.apply_typography_property("white-space", "unsupported");
        assert_eq!(style.white_space, CssWhiteSpace::Normal);
    }

    #[test]
    fn text_align_handles_center_and_right_and_unknown() {
        let mut style = CssStyle::browser_default();
        style.apply_typography_property("text-align", "end");
        assert_eq!(style.text_align, CssTextAlign::End);
        style.apply_typography_property("text-align", "left");
        assert_eq!(style.text_align, CssTextAlign::Start);

        style.apply_typography_property("text-align", "unsupported");
        assert_eq!(style.text_align, CssTextAlign::Start);
    }

    #[test]
    fn letter_spacing_resolves_normal_and_signed_values() {
        let mut style = CssStyle::browser_default();

        style.apply_typography_property("letter-spacing", "normal");
        assert_eq!(style.letter_spacing, 0.0);

        style.apply_typography_property("letter-spacing", "-0.5px");
        assert_eq!(style.letter_spacing, FONT_SPACING_NEGATIVE_HALF);
    }

    #[test]
    fn text_transform_is_parsed_inherited_and_applied_to_unicode_text() {
        let mut style = CssStyle::browser_default();
        style.apply_typography_property("text-transform", "uppercase");
        assert_eq!(style.text_transform, CssTextTransform::Uppercase);
        assert_eq!(style.transformed_text("LibreChat ß"), "LIBRECHAT SS");

        let inherited = CssStyle::from_attributes(&[], &style);
        assert_eq!(inherited.text_transform, CssTextTransform::Uppercase);

        style.apply_typography_property("text-transform", "capitalize");
        assert_eq!(style.transformed_text("hello rust"), "Hello Rust");
        style.apply_typography_property("text-transform", "lowercase");
        assert_eq!(style.transformed_text("KRR"), "krr");
        style.apply_typography_property("text-transform", "unsupported");
        assert_eq!(style.transformed_text("KRR"), "krr");
    }
}
