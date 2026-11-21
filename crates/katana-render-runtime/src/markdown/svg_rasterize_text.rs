use super::text_shaping::{
    FontShape, TextSpacingTrim, shaped_text_width_in_database_with_trim,
    shaped_text_width_with_trim,
};
use super::{HtmlTextMeasurementToken, SvgRasterizeError};
use resvg::usvg;
use std::sync::Arc;

#[path = "svg_rasterize_text_width_fallback.rs"]
mod width_fallback;
#[cfg(test)]
use super::escape_xml_attribute;
use width_fallback::fallback_text_width;
#[cfg(test)]
use width_fallback::{canonical_svg_font_family, escape_xml_text};

pub(super) struct HtmlTextMeasure<'a> {
    font_family: &'a str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&'a str>,
    trim: TextSpacingTrim,
}

impl<'a> HtmlTextMeasure<'a> {
    pub(super) fn new(
        font_family: &'a str,
        font_size: f32,
        font_style: (u16, bool),
        letter_spacing: f32,
        font_feature_settings: Option<&'a str>,
    ) -> Self {
        Self {
            font_family,
            font_size,
            font_weight: font_style.0,
            italic: font_style.1,
            letter_spacing,
            font_feature_settings,
            trim: TextSpacingTrim::Normal,
        }
    }

    pub(super) fn with_trim(mut self, trim: TextSpacingTrim) -> Self {
        self.trim = trim;
        self
    }
}

#[path = "svg_rasterize_text_prepared.rs"]
mod prepared;
pub(super) use prepared::PreparedHtmlTextMeasurement;

pub(super) fn measure_html_text(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Result<f32, SvgRasterizeError> {
    measure_html_text_with_trim(
        text,
        font_family,
        font_size,
        (font_weight, italic),
        letter_spacing,
        font_feature_settings,
        TextSpacingTrim::Normal,
    )
}

pub(super) fn measure_html_text_with_trim(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_style: (u16, bool),
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
    trim: TextSpacingTrim,
) -> Result<f32, SvgRasterizeError> {
    HtmlTextMeasure::new(
        font_family,
        font_size,
        font_style,
        letter_spacing,
        font_feature_settings,
    )
    .with_trim(trim)
    .measure(text)
}

#[cfg(test)]
pub(super) fn measure_html_text_with_cache_status(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Result<(f32, bool), SvgRasterizeError> {
    HtmlTextMeasure {
        font_family,
        font_size,
        font_weight,
        italic,
        letter_spacing,
        font_feature_settings,
        trim: TextSpacingTrim::Normal,
    }
    .measure_with_status(text)
}

impl HtmlTextMeasure<'_> {
    fn measure(&self, text: &str) -> Result<f32, SvgRasterizeError> {
        self.measure_with_status(text).map(|(width, _)| width)
    }

    fn measure_with_status(&self, text: &str) -> Result<(f32, bool), SvgRasterizeError> {
        self.measure_with_status_in_database(text, None)
    }

    fn measure_with_status_in_database(
        &self,
        text: &str,
        database: Option<&Arc<usvg::fontdb::Database>>,
    ) -> Result<(f32, bool), SvgRasterizeError> {
        if text.is_empty() {
            return Ok((0.0, false));
        }
        let shaped = database.map_or_else(
            || self.shaped_measure(text),
            |database| {
                shaped_text_width_in_database_with_trim(
                    database,
                    text,
                    self.font_family,
                    FontShape::new(self.font_size, self.font_weight, self.italic)
                        .with_trim(self.trim),
                    self.letter_spacing,
                    self.font_feature_settings,
                )
            },
        );
        shaped.map_or_else(
            || self.fallback_measure(text).map(|width| (width, false)),
            |width| Ok((width, true)),
        )
    }

    fn shaped_measure(&self, text: &str) -> Option<f32> {
        shaped_text_width_with_trim(
            text,
            self.font_family,
            FontShape::new(self.font_size, self.font_weight, self.italic).with_trim(self.trim),
            self.letter_spacing,
            self.font_feature_settings,
        )
    }

    fn fallback_measure(&self, text: &str) -> Result<f32, SvgRasterizeError> {
        fallback_text_width(
            text,
            self.font_family,
            self.font_size,
            self.font_weight,
            self.italic,
            self.letter_spacing,
        )
    }
}

#[cfg(test)]
#[path = "svg_rasterize_text_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_text_bbox_probe.rs"]
mod bbox_probe;

#[cfg(test)]
#[path = "svg_rasterize_text_html_png_probe.rs"]
mod html_png_probe;

#[cfg(test)]
#[path = "svg_rasterize_text_probe_test_support.rs"]
pub(super) mod probe_test_support;

#[cfg(test)]
#[path = "svg_rasterize_text_probe_file_contract_tests.rs"]
mod probe_file_contract_tests;
