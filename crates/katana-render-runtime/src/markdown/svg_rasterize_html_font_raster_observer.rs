#[path = "svg_rasterize_html_font_local_missing_fixture_tests.rs"]
mod local_missing_fixture;
pub(super) use local_missing_fixture::{
    Scope as LocalMissingFontScope, after_admission as local_font_after_admission,
    after_selector as local_missing_after_selector, take_fixture as local_missing_font_fixture,
};

#[path = "svg_rasterize_html_font_raster_observer_events.rs"]
mod events;

pub(super) use events::{
    admission_result, eager_attempt, eager_reparse, eager_result, install_eager, known_attempt,
    known_fallback, known_result, known_return, known_selector, prepare_ascii_unavailable,
    prepare_known_unavailable, prepare_ready, prepare_unavailable, reject_family, reject_stretch,
    reject_style,
};

#[path = "svg_rasterize_html_font_raster_observer_counters.rs"]
mod counters;
#[path = "svg_rasterize_html_font_raster_observer_hooks.rs"]
mod hooks;
#[path = "svg_rasterize_html_font_raster_observer_source_opens.rs"]
mod source_opens;
#[path = "svg_rasterize_html_font_raster_observer_work.rs"]
mod work;
pub(in crate::markdown::svg_rasterize) use counters::{CacheState, Metric, SlotState};
pub(in crate::markdown::svg_rasterize) use hooks::AttemptScope;
pub(crate) use hooks::FrameScope;
pub(in crate::markdown::svg_rasterize) use hooks::{eager_tree, source_open, work_value};

use std::cell::{Cell, RefCell};
use std::time::Instant;

const MAX_EVENTS: usize = 256;
const MAX_EVENT_JSON_BYTES: usize = 4096;

#[derive(Clone, Default)]
pub(super) struct Snapshot {
    pub(super) events: Vec<serde_json::Value>,
    pub(super) dropped: usize,
    counters: counters::Counters,
    work: work::Work,
}

thread_local! {
    static ACTIVE: RefCell<Option<Snapshot>> = const { RefCell::new(None) };
}

impl Snapshot {
    pub(super) fn operation_counters(&self) -> serde_json::Value {
        serde_json::json!({"operations": self.counters.json(), "work": self.work.json()})
    }
    pub(super) fn complete(&self) -> bool {
        self.dropped == 0 && self.counters.complete() && self.work.complete()
    }
}

pub(in crate::markdown::svg_rasterize) fn enabled() -> bool {
    ACTIVE.with(|active| active.borrow().is_some())
}

pub(in crate::markdown::svg_rasterize) fn count(metric: Metric) {
    ACTIVE.with(|active| {
        if let Some(snapshot) = active.borrow_mut().as_mut() {
            snapshot.counters.bump(metric);
        }
    });
}

pub(in crate::markdown::svg_rasterize) fn record_work(domain: &str, bytes: &[u8]) {
    ACTIVE.with(|active| {
        if let Some(snapshot) = active.borrow_mut().as_mut() {
            snapshot.work.record(domain, bytes);
        }
    });
}

pub(in crate::markdown::svg_rasterize) fn cache_state(state: CacheState) {
    ACTIVE.with(|active| {
        if let Some(snapshot) = active.borrow_mut().as_mut() {
            snapshot.counters.cache_state(state);
        }
    });
}

pub(super) struct Scope {
    previous: Option<Snapshot>,
}
impl Scope {
    pub(super) fn enter() -> Self {
        Self {
            previous: ACTIVE.with(|active| active.replace(Some(Snapshot::default()))),
        }
    }
    pub(super) fn snapshot(&self) -> Snapshot {
        ACTIVE.with(|active| match active.borrow().as_ref() {
            Some(snapshot) => snapshot.clone(),
            None => Snapshot::default(),
        })
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|active| {
            active.replace(self.previous.take());
        });
    }
}

pub(super) fn record(event: impl FnOnce() -> serde_json::Value) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(snapshot) = active.as_mut() else {
            return;
        };
        if snapshot.events.len() >= MAX_EVENTS {
            snapshot.dropped += 1;
            return;
        }
        let event = event();
        if event.to_string().len() > MAX_EVENT_JSON_BYTES {
            snapshot.dropped += 1;
            return;
        }
        if let Some(bytes) = hooks::route(&event) {
            snapshot.work.record("route", &bytes);
        }
        snapshot.events.push(event);
    });
}

#[derive(Default)]
pub(super) struct TreeParseTiming {
    elapsed_ns: Cell<u128>,
}

impl TreeParseTiming {
    pub(super) fn tree(
        &self,
        svg: &str,
        options: &resvg::usvg::Options<'_>,
    ) -> Result<resvg::usvg::Tree, resvg::usvg::Error> {
        self.measure(|| {
            hooks::parse(Metric::KnownParse, svg, || {
                resvg::usvg::Tree::from_str(svg, options)
            })
        })
    }

    pub(super) fn measure<T>(&self, parse: impl FnOnce() -> T) -> T {
        if !ACTIVE.with(|active| active.borrow().is_some()) {
            return parse();
        }
        let started = Instant::now();
        let result = parse();
        self.elapsed_ns
            .set(self.elapsed_ns.get() + started.elapsed().as_nanos());
        result
    }
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_raster_observer_boundary_tests.rs"]
mod boundary_tests;

#[cfg(test)]
mod parse_timing_tests {
    use super::{Scope, TreeParseTiming};

    #[test]
    fn tree_parse_records_success_and_failure_only_with_active_observer() {
        let timing = TreeParseTiming::default();
        let options = resvg::usvg::Options::default();
        let valid = "<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'/>";
        assert!(timing.tree(valid, &options).is_ok());
        assert_eq!(timing.elapsed_ns.get(), 0);

        let observer = Scope::enter();
        assert!(timing.tree(valid, &options).is_ok());
        assert!(timing.tree("<svg", &options).is_err());
        let snapshot = observer.snapshot();
        assert_eq!(
            snapshot.operation_counters()["operations"]["counts"]["known_parse"],
            2
        );
        assert!(snapshot.complete());
    }
}
