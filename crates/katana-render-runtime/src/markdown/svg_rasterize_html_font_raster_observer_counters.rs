use serde_json::{Map, Value};

#[derive(Clone, Copy)]
#[repr(usize)]
pub(in crate::markdown::svg_rasterize) enum Metric {
    KnownParse,
    EagerParse,
    RasterAttempt,
    RasterSuccess,
    KnownSelector,
    EagerSelector,
    KnownFallback,
    EagerFallback,
    AliasAppend,
    AliasReuse,
    AttemptBegin,
    AttemptDrop,
    SourceGet,
    SourceFirstOpenAttempt,
    SourceFirstLoadSuccess,
    SourceOccupiedGet,
    ProofCurrent,
    ProofSelectedCurrent,
    PositiveObserve,
    PositiveFreshEvaluation,
    AbsentMetadata,
    StampMemoHit,
    StampMemoMiss,
    StampMetadata,
    MetricsPrimaryHit,
    MetricsSpareHit,
    MetricsMiss,
    MetricsLoad,
    BudgetEviction,
    SourceGetCacheInactive,
    SourceGetMatchingMetricsOwner,
    LazyDirectoryOpenAttempt,
    LazyTableOpenAttempt,
}

const METRIC_COUNT: usize = Metric::LazyTableOpenAttempt as usize + 1;
const MAX_RETAINED_SOURCE_BYTES: usize = 64 * 1024 * 1024;

const NAMES: [&str; METRIC_COUNT] = [
    "known_parse",
    "eager_parse",
    "raster_attempt",
    "raster_success",
    "known_selector",
    "eager_selector",
    "known_fallback",
    "eager_fallback",
    "alias_append",
    "alias_reuse",
    "attempt_begin",
    "attempt_drop",
    "source_get",
    "source_first_open_attempt",
    "source_first_load_success",
    "source_occupied_get",
    "proof_current",
    "proof_selected_current",
    "positive_observe",
    "positive_fresh_evaluation",
    "absent_metadata",
    "stamp_memo_hit",
    "stamp_memo_miss",
    "stamp_metadata",
    "metrics_primary_hit",
    "metrics_spare_hit",
    "metrics_miss",
    "metrics_load_attempt",
    "budget_admission_eviction",
    "source_get_cache_inactive",
    "source_get_matching_metrics_owner",
    "lazy_directory_open_attempt",
    "lazy_table_open_attempt",
];

#[derive(Clone)]
pub(in crate::markdown::svg_rasterize) struct Counters {
    values: [u64; METRIC_COUNT],
    overflow: bool,
    max_depth: usize,
    max_retained_bytes: usize,
    max_slots: usize,
    terminal: CacheState,
}

impl Default for Counters {
    fn default() -> Self {
        Self {
            values: [0; METRIC_COUNT],
            overflow: false,
            max_depth: 0,
            max_retained_bytes: 0,
            max_slots: 0,
            terminal: CacheState::default(),
        }
    }
}

#[derive(Clone, Default, serde::Serialize)]
pub(in crate::markdown::svg_rasterize) struct CacheState {
    pub(in crate::markdown::svg_rasterize) depth: usize,
    pub(in crate::markdown::svg_rasterize) slots: [SlotState; 2],
}

#[derive(Clone, Default, serde::Serialize)]
pub(in crate::markdown::svg_rasterize) struct SlotState {
    pub(in crate::markdown::svg_rasterize) occupied: bool,
    pub(in crate::markdown::svg_rasterize) retained_bytes: usize,
    pub(in crate::markdown::svg_rasterize) flat_blob: bool,
    pub(in crate::markdown::svg_rasterize) table_function: bool,
    pub(in crate::markdown::svg_rasterize) poisoned: bool,
    pub(in crate::markdown::svg_rasterize) durable: bool,
}

impl CacheState {
    pub(in crate::markdown::svg_rasterize) fn bytes(&self) -> usize {
        self.slots.iter().map(|slot| slot.retained_bytes).sum()
    }
    pub(in crate::markdown::svg_rasterize) fn occupied(&self) -> usize {
        self.slots.iter().filter(|slot| slot.occupied).count()
    }
    pub(in crate::markdown::svg_rasterize) fn released(&self) -> bool {
        self.depth == 0 && self.bytes() == 0 && self.occupied() == 0
    }
}

impl Counters {
    pub(super) fn bump(&mut self, metric: Metric) {
        let value = &mut self.values[metric as usize];
        match value.checked_add(1) {
            Some(next) => *value = next,
            None => self.overflow = true,
        }
    }

    pub(super) fn cache_state(&mut self, state: CacheState) {
        self.max_depth = self.max_depth.max(state.depth);
        self.max_retained_bytes = self.max_retained_bytes.max(state.bytes());
        self.max_slots = self.max_slots.max(state.occupied());
        self.terminal = state;
    }

    pub(super) fn complete(&self) -> bool {
        !self.overflow
            && self.max_slots <= 2
            && self.max_retained_bytes <= MAX_RETAINED_SOURCE_BYTES
            && self.terminal.released()
    }

    pub(in crate::markdown::svg_rasterize) fn json(&self) -> Value {
        let mut counts = Map::new();
        for (name, count) in NAMES.iter().zip(self.values) {
            counts.insert((*name).to_string(), Value::from(count));
        }
        serde_json::json!({
            "schema": 2, "counts": counts, "overflow": self.overflow,
            "maxActiveDepth": self.max_depth,
            "maxCacheRetainedSourceBytes": self.max_retained_bytes,
            "maxOccupiedSlots": self.max_slots, "terminalCacheState": self.terminal,
            "cacheOwnerMeaning": "two_entry_slots_not_arc_reference_count",
            "physicalSyscallCountObserved": false,
            "timingAcceptance": false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Counters, Metric, NAMES};

    #[test]
    fn diagnostic_metric_overflow_rejects_an_incomplete_capture() {
        let mut counters = Counters::default();
        let metric = Metric::KnownParse;
        let index = metric as usize;
        counters.values[index] = u64::MAX - 1;
        for (overflow, complete) in [(false, true), (true, false)] {
            counters.bump(metric);
            let report = counters.json();
            assert_eq!(report["counts"][NAMES[index]].as_u64(), Some(u64::MAX));
            assert_eq!(report["overflow"], overflow);
            assert_eq!(counters.complete(), complete);
        }
    }
}
