use super::super::html_font_db_for_text;
use super::{
    HtmlFontRequest, HtmlTextMeasurementToken, MAX_FAMILY_BYTES, MemoEntry, TOKEN_STORE,
    TokenStore, active_memo_full, active_memo_result, remember_active, render_generations,
};

impl HtmlTextMeasurementToken {
    pub(crate) fn for_request(text: &str, font_family: &str) -> Option<HtmlTextMeasurementToken> {
        if font_family.len() > MAX_FAMILY_BYTES || !super::super::font::memo_usable() {
            return None;
        }
        let request = HtmlFontRequest::from_text(font_family, text);
        if let Some(result) = active_memo_result(&request) {
            return result;
        }
        if active_memo_full() {
            return None;
        }
        let database = html_font_db_for_text(font_family, text);
        let Some(generations) = render_generations(&database) else {
            remember_active(MemoEntry::Miss(request));
            return None;
        };
        if let Some(token) =
            TOKEN_STORE.with(|store| store.borrow().get(&request, &database, &generations))
        {
            remember_active(MemoEntry::Hit(request, token.clone()));
            return Some(token);
        }
        let Some((token, bytes)) = TokenStore::build_token(&request, database, generations) else {
            remember_active(MemoEntry::Miss(request));
            return None;
        };
        remember_active(MemoEntry::Hit(request.clone(), token.clone()));
        TOKEN_STORE.with(|store| store.borrow_mut().insert(request, token.clone(), bytes));
        Some(token)
    }
}
