use resvg::usvg::fontdb::{Database, ID};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Weak},
};

#[path = "svg_rasterize_text_family_scope_cache/store.rs"]
mod store;

#[cfg(test)]
#[path = "svg_rasterize_text_family_scope_cache_capacity_tests.rs"]
mod capacity_tests;

#[cfg(test)]
#[path = "svg_rasterize_text_family_scope_cache_lifecycle_tests.rs"]
mod lifecycle_tests;

const MAX_CONTEXTS: usize = 16;
const MAX_CONTEXT_CHARS: usize = 256;
const MAX_TOTAL_ENTRIES: usize = 4096;
const MAX_FAMILY_BYTES: usize = 4096;
const MAX_TOTAL_FAMILY_BYTES: usize = 64 * 1024;

struct ContextKey {
    database: Weak<Database>,
    base: ID,
    family: Box<str>,
    weight: u16,
    italic: bool,
}

struct Context {
    key: ContextKey,
    selections: HashMap<char, ID>,
    insertion_order: VecDeque<char>,
}

#[derive(Default)]
struct Cache {
    contexts: Vec<Context>,
    total_entries: usize,
    total_family_bytes: usize,
}

thread_local! {
    static ACTIVE: RefCell<Option<Cache>> = const { RefCell::new(None) };
}

pub(super) struct Scope {
    previous: Option<Cache>,
    finished: bool,
}

impl Scope {
    pub(super) fn enter() -> Self {
        let previous = ACTIVE.with(|active| active.replace(Some(Cache::default())));
        Self {
            previous,
            finished: false,
        }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if !self.finished {
            ACTIVE.with(|active| active.replace(self.previous.take()));
            self.finished = true;
        }
    }
}

pub(super) fn lookup_runs(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    weight: u16,
    italic: bool,
    text: &str,
) -> Option<Vec<(ID, String)>> {
    with_matching_context(database, base, family, weight, italic, |context| {
        let mut result = Vec::new();
        for character in text.chars() {
            let selected = context.selections.get(&character).copied()?;
            super::font::append_font_run(&mut result, selected, character);
        }
        Some(result)
    })
}

pub(super) fn lookup_known(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    weight: u16,
    italic: bool,
    text: &str,
) -> Option<(HashMap<char, ID>, String)> {
    with_matching_context(database, base, family, weight, italic, |context| {
        let (known, missing) = known_selection_for_text(context, text)?;
        Some((known, missing))
    })
}

fn with_matching_context<T>(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    weight: u16,
    italic: bool,
    operation: impl FnOnce(&Context) -> Option<T>,
) -> Option<T> {
    if !super::font::memo_usable() || !cacheable_family(family) {
        return None;
    }
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let cache = active.as_mut()?;
        purge_dead_contexts(cache);
        let context = cache
            .contexts
            .iter()
            .find(|context| same_context(&context.key, database, base, family, weight, italic))?;
        operation(context)
    })
}

fn known_selection_for_text(context: &Context, text: &str) -> Option<(HashMap<char, ID>, String)> {
    let mut known = HashMap::new();
    let mut missing = String::new();
    let mut seen = HashSet::new();
    for character in text.chars() {
        if !seen.insert(character) {
            continue;
        }
        if seen.len() > MAX_CONTEXT_CHARS {
            return None;
        }
        if let Some(selected) = context.selections.get(&character) {
            known.insert(character, *selected);
        } else {
            missing.push(character);
        }
    }
    Some((known, missing))
}

pub(super) fn store(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    weight: u16,
    italic: bool,
    selections: impl IntoIterator<Item = (char, ID)>,
) {
    store::store(database, base, family, weight, italic, selections);
}

fn same_context(
    key: &ContextKey,
    database: &Arc<Database>,
    base: ID,
    family: &str,
    weight: u16,
    italic: bool,
) -> bool {
    key.base == base
        && key.family.as_ref() == family
        && key.weight == weight
        && key.italic == italic
        && key
            .database
            .upgrade()
            .is_some_and(|cached| Arc::ptr_eq(&cached, database))
}

fn purge_dead_contexts(cache: &mut Cache) {
    let mut removed_entries = 0;
    let mut removed_family_bytes = 0;
    cache.contexts.retain(|context| {
        if context.key.database.strong_count() == 0 {
            removed_entries += context.selections.len();
            removed_family_bytes += context.key.family.len();
            false
        } else {
            true
        }
    });
    cache.total_entries -= removed_entries;
    cache.total_family_bytes -= removed_family_bytes;
}

fn cacheable_family(family: &str) -> bool {
    family.len() <= MAX_FAMILY_BYTES
}
