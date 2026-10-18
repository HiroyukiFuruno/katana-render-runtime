use super::{HtmlTextMeasure, HtmlTextMeasurementToken, SvgRasterizeError};
use resvg::usvg;
use std::sync::Arc;

pub(crate) struct PreparedHtmlTextMeasurement<'a> {
    text: &'a str,
    style: HtmlTextMeasure<'a>,
    database: Arc<usvg::fontdb::Database>,
    token: Option<HtmlTextMeasurementToken>,
}

impl<'a> PreparedHtmlTextMeasurement<'a> {
    pub(in crate::markdown::svg_rasterize) fn prepare(
        text: &'a str,
        font_family: &'a str,
        font_size: f32,
        font_weight: u16,
        italic: bool,
        letter_spacing: f32,
        font_feature_settings: Option<&'a str>,
    ) -> Self {
        let selected =
            super::super::font::measured_database(font_family, text, font_weight, italic);
        let token = HtmlTextMeasurementToken::for_selected_database(
            font_family,
            &selected.key,
            &selected.database,
        );
        Self {
            text,
            style: HtmlTextMeasure {
                font_family,
                font_size,
                font_weight,
                italic,
                letter_spacing,
                font_feature_settings,
            },
            database: selected.database,
            token,
        }
    }

    #[cfg(test)]
    pub(super) fn database_snapshot(&self) -> &Arc<usvg::fontdb::Database> {
        &self.database
    }

    pub(crate) fn token(&self) -> Option<&HtmlTextMeasurementToken> {
        self.token.as_ref()
    }

    pub(crate) fn measure_with_cache_status(&self) -> Result<(f32, bool), SvgRasterizeError> {
        /* WHY: token取得後の再選択で、別世代の幅を前のtokenへ保存しない。 */
        self.style
            .measure_with_status_in_database(self.text, Some(&self.database))
    }
}
