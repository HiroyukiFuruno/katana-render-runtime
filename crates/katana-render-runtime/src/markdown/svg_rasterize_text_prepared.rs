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
        style: HtmlTextMeasure<'a>,
    ) -> Self {
        let selected = selected_database(text, style.font_family, style.font_weight, style.italic);
        Self::from_selected(text, style, selected)
    }

    fn from_selected(
        text: &'a str,
        style: HtmlTextMeasure<'a>,
        selected: super::super::font::MeasuredDatabase,
    ) -> Self {
        let token = HtmlTextMeasurementToken::for_selected_database(
            style.font_family,
            &selected.key,
            &selected.database,
        );
        Self {
            text,
            style,
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

fn selected_database(
    text: &str,
    family: &str,
    weight: u16,
    italic: bool,
) -> super::super::font::MeasuredDatabase {
    super::super::font::measured_database(family, text, weight, italic)
}
