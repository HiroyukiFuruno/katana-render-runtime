use super::{
    Database, DatabaseKey, HtmlTextMeasurementToken, MemoEntry, TOKEN_STORE, TokenStore,
    remember_active, render_generations,
};
use std::sync::Arc;

pub(super) fn token_for_selected(
    key: DatabaseKey,
    database: Arc<Database>,
) -> Option<HtmlTextMeasurementToken> {
    let Some(generations) = render_generations(&database) else {
        remember_active(MemoEntry::Miss(key));
        return None;
    };
    if let Some(token) =
        TOKEN_STORE.with(|store| store.borrow().get_key(&key, &database, &generations))
    {
        remember_active(MemoEntry::Hit(key, token.clone()));
        return Some(token);
    }
    let Some((token, bytes)) = TokenStore::build_token(&key.request, database, generations) else {
        remember_active(MemoEntry::Miss(key));
        return None;
    };
    remember_active(MemoEntry::Hit(key.clone(), token.clone()));
    TOKEN_STORE.with(|store| store.borrow_mut().insert_key(key, token.clone(), bytes));
    Some(token)
}
