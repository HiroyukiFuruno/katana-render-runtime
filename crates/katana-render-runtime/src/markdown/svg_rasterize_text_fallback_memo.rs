use resvg::usvg::{self, fontdb};
use std::{mem, sync::Arc};

#[path = "svg_rasterize_text_fallback_memo_cache.rs"]
mod cache;
#[path = "svg_rasterize_text_fallback_memo_entries.rs"]
mod entries;
#[path = "svg_rasterize_text_fallback_memo_scope.rs"]
mod scope;

#[cfg(test)]
const MAX_SELECTOR_ENTRIES: usize = entries::MAX_ENTRIES;

pub(in super::super) fn install(options: &mut usvg::Options<'_>) {
    let selector = mem::replace(
        &mut options.font_resolver.select_fallback,
        usvg::FontResolver::default_fallback_selector(),
    );
    options.font_resolver.select_fallback = Box::new(move |character, excluded_ids, database| {
        match cache::lookup(database, character, excluded_ids) {
            cache::Lookup::Hit(selection) => selection,
            cache::Lookup::Miss(key) => {
                select_and_store(&selector, database, character, excluded_ids, key)
            }
            cache::Lookup::Bypass => selector(character, excluded_ids, database),
        }
    });
}

fn select_and_store(
    selector: &usvg::FallbackSelectionFn<'_>,
    database: &mut Arc<fontdb::Database>,
    character: char,
    excluded_ids: &[fontdb::ID],
    key: cache::SelectorKey,
) -> Option<fontdb::ID> {
    let selection = selector(character, excluded_ids, database);
    entries::store(database, key, selection);
    selection
}

pub(super) fn with_attempt<T>(operation: impl FnOnce() -> T) -> T {
    scope::with_attempt(operation)
}

pub(in super::super) fn with_validated_tree_parse<T>(mut parse: impl FnMut() -> T) -> T {
    super::font::with_validated_stamp_batch(|| with_attempt(&mut parse))
}

#[cfg(test)]
#[path = "svg_rasterize_text_fallback_memo_state_tests.rs"]
mod state_tests;
#[cfg(test)]
#[path = "svg_rasterize_text_fallback_memo_tests.rs"]
mod tests;
