use super::{ACTIVE_SCOPE, Entry, entry::CacheKey};
use crate::markdown::svg_rasterize::font::observation::{CacheState, SlotState};
pub(super) use crate::markdown::svg_rasterize::font::observation::{Metric, count};
use crate::markdown::svg_rasterize::text_shaping::FileStamp;
use std::{path::Path, sync::atomic::Ordering};

fn slot(entry: Option<&Entry>) -> SlotState {
    let Some(entry) = entry else {
        return SlotState::default();
    };
    SlotState {
        occupied: true,
        retained_bytes: entry.font.retained_source_bytes,
        flat_blob: matches!(entry.font.font.source(), harfrust::font::Source::Blob(_)),
        table_function: matches!(
            entry.font.font.source(),
            harfrust::font::Source::TableFunction(_)
        ),
        poisoned: entry
            .font
            .poisoned
            .as_ref()
            .is_some_and(|poison| poison.load(Ordering::Acquire)),
        durable: match &entry.key {
            CacheKey::File { generation, .. } => generation.durable_reusable(),
            CacheKey::Binary { .. } => true,
        },
    }
}

pub(in crate::markdown::svg_rasterize) fn state() -> CacheState {
    ACTIVE_SCOPE.with(|scope| {
        let scope = scope.borrow();
        CacheState {
            depth: scope.depth,
            slots: [slot(scope.entry.as_ref()), slot(scope.spare.as_ref())],
        }
    })
}

pub(super) fn publish() {
    crate::markdown::svg_rasterize::font::observation::cache_state(state());
}

pub(in crate::markdown::svg_rasterize) fn source_get(path: &Path, stamp: &FileStamp) {
    ACTIVE_SCOPE.with(|scope| {
        let scope = scope.borrow();
        if scope.depth == 0 {
            count(Metric::SourceGetCacheInactive);
        }
        if scope.depth != 0
            && scope
                .entry
                .iter()
                .chain(scope.spare.iter())
                .any(|entry| eligible(entry, path, stamp))
        {
            count(Metric::SourceGetMatchingMetricsOwner);
        }
    });
}

pub(super) fn eligible(entry: &Entry, path: &Path, stamp: &FileStamp) -> bool {
    matches!(&entry.key, CacheKey::File { path: cached, generation, .. }
        if cached == path && generation.diagnostic_matches_stamp(stamp))
        && !entry
            .font
            .poisoned
            .as_ref()
            .is_some_and(|poison| poison.load(Ordering::Acquire))
}

pub(super) struct Mutation {
    before: usize,
    budget: bool,
}
impl Mutation {
    pub(super) fn new(budget: bool) -> Self {
        Self {
            before: state().occupied(),
            budget,
        }
    }
}
impl Drop for Mutation {
    fn drop(&mut self) {
        let state = state();
        if self.budget {
            for _ in state.occupied()..self.before {
                count(Metric::BudgetEviction);
            }
        }
        crate::markdown::svg_rasterize::font::observation::cache_state(state);
    }
}
