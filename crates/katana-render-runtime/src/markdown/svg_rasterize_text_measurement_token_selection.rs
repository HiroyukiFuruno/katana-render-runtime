use super::{
    ACTIVE_RENDER_MEMO, Database, DatabaseKey, HtmlTextMeasurementToken, MemoEntry, TOKEN_STORE,
    TokenStore, active_memo_result, remember_active, render_generations,
};
use std::sync::Arc;

pub(super) fn token_for_selected(
    key: DatabaseKey,
    database: Arc<Database>,
) -> Option<HtmlTextMeasurementToken> {
    let Some(generations) = render_generations(&database) else {
        remember_mode_miss(key);
        return None;
    };
    if let Some(token) =
        TOKEN_STORE.with(|store| store.borrow().get_key(&key, &database, &generations))
    {
        remember_mode_hit(key, token.clone());
        return Some(token);
    }
    let Some((token, bytes)) = TokenStore::build_token(&key.request, database, generations) else {
        remember_mode_miss(key);
        return None;
    };
    remember_mode_hit(key.clone(), token.clone());
    TOKEN_STORE.with(|store| store.borrow_mut().insert_key(key, token.clone(), bytes));
    Some(token)
}

fn remember_mode_miss(key: DatabaseKey) {
    if key.discovery == super::super::super::font::DiscoveryMode::Eager {
        remember_active(MemoEntry::Miss(key.request));
    } else {
        remember_active(MemoEntry::ModeMiss(key));
    }
}

fn remember_mode_hit(key: DatabaseKey, token: HtmlTextMeasurementToken) {
    if key.discovery == super::super::super::font::DiscoveryMode::Eager {
        remember_active(MemoEntry::Hit(key.request, token));
    } else {
        remember_active(MemoEntry::ModeHit(key, token));
    }
}

pub(super) fn mode_memo_result(key: &DatabaseKey) -> Option<Option<HtmlTextMeasurementToken>> {
    if key.discovery == super::super::super::font::DiscoveryMode::Eager {
        return active_memo_result(&key.request);
    }
    ACTIVE_RENDER_MEMO.with(|memo| {
        memo.borrow()
            .as_ref()?
            .iter()
            .find_map(|entry| match entry {
                MemoEntry::ModeMiss(cached) if cached == key => Some(None),
                MemoEntry::ModeHit(cached, token) if cached == key => Some(Some(token.clone())),
                _ => None,
            })
    })
}
