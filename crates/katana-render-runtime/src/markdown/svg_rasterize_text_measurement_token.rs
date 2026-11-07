use super::super::font::{DatabaseKey, measured_database};
use super::{FontSourceGeneration, HtmlFontRequest, font_source_generation};
use resvg::usvg::fontdb::{Database, ID};
use std::{
    cell::RefCell,
    marker::PhantomData,
    rc::Rc,
    sync::{Arc, Weak as DbWeak},
};

const MAX_TOKENS: usize = 8;
const MAX_FAMILY_BYTES: usize = 4096;
const MAX_TOKEN_FACES: usize = 512;

#[path = "svg_rasterize_text_measurement_token_store.rs"]
mod token_store;
use token_store::TokenStore;
#[path = "svg_rasterize_text_measurement_token_selection.rs"]
mod selection;
use selection::{mode_memo_result, token_for_selected};

#[path = "svg_rasterize_text_measurement_token_key.rs"]
mod key;
pub(crate) use key::HtmlTextMeasurementTokenKey;

struct Proof {
    database: DbWeak<Database>,
    generations: Vec<(ID, FontSourceGeneration)>,
    _not_send_sync: PhantomData<Rc<()>>,
}

#[derive(Clone)]
pub(crate) struct HtmlTextMeasurementToken {
    proof: Rc<Proof>,
}

enum MemoEntry {
    Miss(HtmlFontRequest),
    Hit(HtmlFontRequest, HtmlTextMeasurementToken),
    ModeMiss(DatabaseKey),
    ModeHit(DatabaseKey, HtmlTextMeasurementToken),
}

thread_local! {
    static TOKEN_STORE: RefCell<token_store::TokenStore> = const { RefCell::new(token_store::TokenStore::new()) };
    static ACTIVE_RENDER_MEMO: RefCell<Option<Vec<MemoEntry>>> = const { RefCell::new(None) };
}

pub(super) struct Scope {
    previous_memo: Option<Vec<MemoEntry>>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl Scope {
    pub(super) fn enter() -> Self {
        let previous_memo = ACTIVE_RENDER_MEMO.with(|memo| memo.replace(Some(Vec::new())));
        Self {
            previous_memo,
            _not_send_sync: PhantomData,
        }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE_RENDER_MEMO.with(|memo| {
            let _ = memo.replace(self.previous_memo.take());
        });
    }
}

impl HtmlTextMeasurementToken {
    pub(crate) fn is_usable(&self) -> bool {
        if !super::font::memo_usable() {
            return false;
        }
        self.proof.database.upgrade().is_some()
    }
}

impl HtmlTextMeasurementToken {
    pub(crate) fn for_measured_text(
        text: &str,
        family: &str,
        weight: u16,
        italic: bool,
    ) -> Option<Self> {
        if family.len() > MAX_FAMILY_BYTES || !super::font::memo_usable() {
            return None;
        }
        let selected = measured_database(family, text, weight, italic);
        Self::for_selected_database(family, &selected.key, &selected.database)
    }

    pub(in crate::markdown::svg_rasterize) fn for_selected_database(
        family: &str,
        key: &DatabaseKey,
        database: &Arc<Database>,
    ) -> Option<Self> {
        if family.len() > MAX_FAMILY_BYTES || !super::font::memo_usable() {
            return None;
        }
        if let Some(result) = mode_memo_result(key) {
            return result.filter(|token| {
                token
                    .proof
                    .database
                    .upgrade()
                    .is_some_and(|cached| Arc::ptr_eq(&cached, database))
            });
        }
        if active_memo_full() {
            return None;
        }
        token_for_selected(key.clone(), Arc::clone(database))
    }
}

fn active_memo_result(request: &HtmlFontRequest) -> Option<Option<HtmlTextMeasurementToken>> {
    ACTIVE_RENDER_MEMO.with(|memo| {
        memo.borrow()
            .as_ref()?
            .iter()
            .find_map(|entry| match entry {
                MemoEntry::Miss(cached) if cached == request => Some(None),
                MemoEntry::Hit(cached, token) if cached == request => Some(Some(token.clone())),
                _ => None,
            })
    })
}

fn active_memo_full() -> bool {
    ACTIVE_RENDER_MEMO.with(|memo| {
        memo.borrow()
            .as_ref()
            .is_none_or(|entries| entries.len() >= MAX_TOKENS)
    })
}

fn render_generations(database: &Arc<Database>) -> Option<Vec<(ID, FontSourceGeneration)>> {
    if database.faces().take(MAX_TOKEN_FACES + 1).count() > MAX_TOKEN_FACES {
        return None;
    }
    let generations = database
        .faces()
        .map(|face| (face.id, font_source_generation(database, face.id)))
        .collect::<Vec<_>>();
    if !super::font::memo_usable() {
        return None;
    }
    generations
        .iter()
        .all(|(_, generation)| generation.durable_reusable())
        .then_some(generations)
}

fn remember_active(entry: MemoEntry) {
    ACTIVE_RENDER_MEMO.with(|memo| {
        if let Some(entries) = memo.borrow_mut().as_mut() {
            entries.push(entry);
        }
    });
}

#[cfg(test)]
#[path = "svg_rasterize_text_measurement_token_legacy.rs"]
mod legacy;

#[cfg(test)]
#[path = "svg_rasterize_text_measurement_token_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_text_measurement_token_lifecycle_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
#[path = "svg_rasterize_text_measurement_token_batch_retry_tests.rs"]
mod batch_retry_tests;

#[cfg(all(test, target_os = "macos"))]
pub(in crate::markdown::svg_rasterize) fn with_selection_for_coverage<T>(
    mut operation: impl FnMut(
        fn(DatabaseKey, Arc<Database>) -> Option<HtmlTextMeasurementToken>,
        fn(&DatabaseKey) -> Option<Option<HtmlTextMeasurementToken>>,
    ) -> T,
) -> T {
    super::font::with_validated_stamp_batch(|| {
        let _scope = Scope::enter();
        operation(token_for_selected, mode_memo_result)
    })
}
