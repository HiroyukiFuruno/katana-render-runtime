use std::{cell::RefCell, marker::PhantomData, mem, rc::Rc, time::Instant};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Stats {
    pub(crate) line_calls: usize,
    pub(crate) line_hits: usize,
    pub(crate) line_misses: usize,
    pub(crate) line_uncached_us: usize,
    pub(crate) baseline_calls: usize,
    pub(crate) baseline_hits: usize,
    pub(crate) baseline_misses: usize,
    pub(crate) baseline_uncached_us: usize,
    pub(crate) line_metric_db_ns: usize,
    pub(crate) line_metric_matching_ns: usize,
    pub(crate) line_metric_runs_ns: usize,
    pub(crate) line_metric_faces_ns: usize,
    pub(crate) line_metric_face_file_calls: usize,
    pub(crate) line_metric_face_binary_calls: usize,
    pub(crate) line_metric_face_shared_calls: usize,
    pub(crate) face_inner: FaceMetricStats,
}

thread_local! {
    static ACTIVE: RefCell<Option<Stats>> = const { RefCell::new(None) };
}

pub(crate) struct Scope {
    previous: Option<Stats>,
    finished: bool,
    _not_send_sync: PhantomData<Rc<()>>,
}

pub(super) fn enter(enabled: bool) -> Scope {
    let previous = ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        mem::replace(&mut *active, enabled.then_some(Stats::default()))
    });
    Scope {
        previous,
        finished: false,
        _not_send_sync: PhantomData,
    }
}

impl Scope {
    pub(crate) fn finish(&mut self) -> Stats {
        if self.finished {
            return Stats::default();
        }
        self.finished = true;
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            let current = mem::replace(&mut *active, self.previous.take());
            current.unwrap_or_else(Stats::default)
        })
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if !self.finished {
            ACTIVE.with(|active| *active.borrow_mut() = self.previous.take());
        }
    }
}

pub(super) fn line_start() -> Option<Instant> {
    ACTIVE.with(|active| active.borrow().is_some().then(Instant::now))
}

pub(super) fn baseline_start() -> Option<Instant> {
    line_start()
}

pub(super) fn line_metrics_detail(enabled: bool) -> Option<LineMetricsDetailScope> {
    enabled.then(LineMetricsDetailScope::default)
}

#[path = "svg_rasterize_face_metric_detail.rs"]
mod face_metric_detail;
pub(crate) use face_metric_detail::FaceMetricStats;
pub(super) use face_metric_detail::{FaceMetricOutcome, FaceMetricPhase, FaceMetricScope};

#[path = "svg_rasterize_line_metrics_detail.rs"]
mod line_metrics_detail;
pub(super) use line_metrics_detail::LineMetricsDetailScope;

pub(super) fn line_finish(hit: bool, started: Option<Instant>) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(stats) = active.as_mut() else {
            return;
        };
        stats.line_calls += 1;
        update_line_stats(stats, hit, started);
    });
}

pub(super) fn baseline_finish(hit: bool, started: Option<Instant>) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(stats) = active.as_mut() else {
            return;
        };
        stats.baseline_calls += 1;
        update_baseline_stats(stats, hit, started);
    });
}

fn update_line_stats(stats: &mut Stats, hit: bool, started: Option<Instant>) {
    if hit {
        stats.line_hits += 1;
    } else {
        stats.line_misses += 1;
        stats.line_uncached_us = add_elapsed(stats.line_uncached_us, started);
    }
}

fn update_baseline_stats(stats: &mut Stats, hit: bool, started: Option<Instant>) {
    if hit {
        stats.baseline_hits += 1;
    } else {
        stats.baseline_misses += 1;
        stats.baseline_uncached_us = add_elapsed(stats.baseline_uncached_us, started);
    }
}

fn add_elapsed(total: usize, started: Option<Instant>) -> usize {
    started.map_or(total, |instant| {
        total.saturating_add(
            instant
                .elapsed()
                .as_micros()
                .try_into()
                .unwrap_or(usize::MAX),
        )
    })
}

#[cfg(test)]
#[path = "svg_rasterize_measurement_diagnostic_tests.rs"]
mod tests;

impl super::SvgRasterizeOps {
    pub(crate) fn begin_html_measurement_diagnostic(enabled: bool) -> Scope {
        enter(enabled)
    }
}
