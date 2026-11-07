use super::{SvgRasterizeOps, TextSpacingTrim, text_shaping};

impl SvgRasterizeOps {
    pub(crate) fn html_text_baseline_for_text(
        text: &str,
        family: &str,
        size: f32,
        weight: u16,
        italic: bool,
        line_height: f32,
        normal_line_height: bool,
    ) -> Option<f32> {
        text_shaping::html_text_baseline_for_text(
            text,
            family,
            size,
            weight,
            italic,
            line_height,
            normal_line_height,
        )
    }
    pub(crate) fn html_line_metrics(
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
        italic: bool,
    ) -> Option<(f32, f32)> {
        text_shaping::html_line_metrics(text, font_family, font_size, font_weight, italic)
    }
    pub(crate) fn shape_html_text_dx_with_trim(
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
        italic: bool,
        font_feature_settings: Option<&str>,
        trim: TextSpacingTrim,
    ) -> Option<Vec<f32>> {
        text_shaping::shaped_html_text_dx_with_trim(
            text,
            font_family,
            font_size,
            font_weight,
            italic,
            font_feature_settings,
            trim,
        )
    }

    pub(crate) fn effective_html_font_size(size: f32) -> f32 {
        text_shaping::effective_html_font_size(size)
    }

    pub(crate) fn html_input_character_metrics(
        family: &str,
        size: f32,
        weight: u16,
        italic: bool,
    ) -> Option<text_shaping::InputCharacterMetrics> {
        text_shaping::html_input_character_metrics(family, size, weight, italic)
    }

    pub(crate) fn html_text_font_runs(
        text: &str,
        font_family: &str,
        font_weight: u16,
        italic: bool,
    ) -> Option<Vec<(String, String)>> {
        text_shaping::html_text_font_runs(text, font_family, font_weight, italic)
    }
}
