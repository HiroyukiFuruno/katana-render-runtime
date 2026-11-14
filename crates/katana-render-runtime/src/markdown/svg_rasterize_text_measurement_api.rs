#[cfg(test)]
use super::HtmlTextMeasurementToken;
use super::{SvgRasterizeError, SvgRasterizeOps, text, text_shaping};
use text_shaping::SvgTextFragmentLayout;

impl SvgRasterizeOps {
    pub(crate) fn measure_svg_fragment_layout(
        fragments: &[String],
        family: &str,
        size: f32,
        weight: u16,
        italic: bool,
    ) -> Result<Option<SvgTextFragmentLayout>, SvgRasterizeError> {
        text_shaping::shaped_svg_fragment_layout(fragments, family, size, weight, italic)
    }
    #[cfg(test)]
    pub(crate) fn prepare_html_text_measurement<'a>(
        text: &'a str,
        font_family: &'a str,
        font_size: f32,
        font_weight: u16,
        italic: bool,
        letter_spacing: f32,
        font_feature_settings: Option<&'a str>,
    ) -> text::PreparedHtmlTextMeasurement<'a> {
        Self::prepare_html_text_measurement_with_trim(
            text,
            font_family,
            font_size,
            (font_weight, italic),
            letter_spacing,
            font_feature_settings,
            text_shaping::TextSpacingTrim::Normal,
        )
    }

    pub(crate) fn prepare_html_text_measurement_with_trim<'a>(
        text: &'a str,
        font_family: &'a str,
        font_size: f32,
        font_style: (u16, bool),
        letter_spacing: f32,
        font_feature_settings: Option<&'a str>,
        trim: text_shaping::TextSpacingTrim,
    ) -> text::PreparedHtmlTextMeasurement<'a> {
        text::PreparedHtmlTextMeasurement::prepare(
            text,
            text::HtmlTextMeasure::new(
                font_family,
                font_size,
                font_style,
                letter_spacing,
                font_feature_settings,
            )
            .with_trim(trim),
        )
    }

    pub(crate) fn with_html_font_face_cache_scope<T>(operation: impl FnOnce() -> T) -> T {
        text_shaping::with_font_face_cache_scope(operation)
    }

    pub(crate) fn with_html_font_render_scope<T>(operation: impl FnMut() -> T) -> T {
        text_shaping::with_render_scoped_stamp_batch(operation)
    }

    pub(crate) fn clear_html_line_metrics_cache() {
        text_shaping::clear_html_line_metrics_cache();
    }

    pub(crate) fn measure_svg_text_metrics(
        text: &str,
        family: &str,
        size: f32,
        weight: u16,
        italic: bool,
    ) -> Result<super::SvgTextRunMetrics, SvgRasterizeError> {
        text_shaping::shaped_svg_text_metrics(text, family, size, weight, italic).ok_or_else(|| {
            SvgRasterizeError::RasterizeFailed("SVG font metrics unavailable".into())
        })
    }

    pub(crate) fn measure_svg_font_x_height(
        family: &str,
        size: f32,
        weight: u16,
        italic: bool,
    ) -> Result<Option<f32>, SvgRasterizeError> {
        text_shaping::shaped_svg_font_x_height(family, size, weight, italic).ok_or_else(|| {
            SvgRasterizeError::RasterizeFailed("SVG font x-height unavailable".into())
        })
    }

    pub(crate) fn measure_html_text(
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
        italic: bool,
        letter_spacing: f32,
        font_feature_settings: Option<&str>,
    ) -> Result<f32, SvgRasterizeError> {
        text::measure_html_text(
            text,
            font_family,
            font_size,
            font_weight,
            italic,
            letter_spacing,
            font_feature_settings,
        )
    }

    #[cfg(test)]
    pub(crate) fn measure_html_text_with_cache_status(
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
        italic: bool,
        letter_spacing: f32,
        font_feature_settings: Option<&str>,
    ) -> Result<(f32, bool), SvgRasterizeError> {
        text::measure_html_text_with_cache_status(
            text,
            font_family,
            font_size,
            font_weight,
            italic,
            letter_spacing,
            font_feature_settings,
        )
    }
}

#[cfg(test)]
impl SvgRasterizeOps {
    pub(crate) fn html_text_measurement_token(
        text: &str,
        font_family: &str,
    ) -> Option<HtmlTextMeasurementToken> {
        text_shaping::HtmlTextMeasurementToken::for_request(text, font_family)
    }
}
