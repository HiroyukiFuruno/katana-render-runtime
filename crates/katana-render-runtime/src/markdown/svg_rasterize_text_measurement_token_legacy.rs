use super::super::html_font_db_for_text;
use super::{
    DatabaseKey, HtmlFontRequest, HtmlTextMeasurementToken, MAX_FAMILY_BYTES, active_memo_full,
    active_memo_result, token_for_selected,
};

impl HtmlTextMeasurementToken {
    pub(crate) fn for_request(text: &str, font_family: &str) -> Option<HtmlTextMeasurementToken> {
        if font_family.len() > MAX_FAMILY_BYTES || !super::super::font::memo_usable() {
            return None;
        }
        let key = DatabaseKey::eager(HtmlFontRequest::from_text(font_family, text));
        if let Some(result) = active_memo_result(&key) {
            return result;
        }
        if active_memo_full() {
            return None;
        }
        token_for_selected(key, html_font_db_for_text(font_family, text))
    }
}
