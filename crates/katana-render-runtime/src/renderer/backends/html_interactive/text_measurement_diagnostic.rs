use std::{cell::RefCell, marker::PhantomData, rc::Rc, time::Instant};

use super::super::html_debug_trace::HtmlDebugTrace;
use super::types::LayoutResult;
use crate::markdown::svg_rasterize::measurement_diagnostic::Stats as SvgMeasurementStats;

#[path = "text_measurement_diagnostic_metrics.rs"]
mod metrics;
#[path = "text_measurement_diagnostic_timing.rs"]
mod timing;

pub(super) use timing::{
    record_style_timing, record_taffy_timing, record_text_total_timing, record_wrap_timing,
    start_style_timing, start_taffy_timing, start_text_total_timing, start_wrap_timing,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TextMeasurementDiagnosticStats {
    pub(super) lookups: usize,
    pub(super) hits: usize,
    pub(super) misses: usize,
    pub(super) measured_calls: usize,
    pub(super) measured_ns: usize,
    pub(super) style_calls: usize,
    pub(super) style_ns: usize,
    pub(super) taffy_calls: usize,
    pub(super) taffy_ns: usize,
    pub(super) text_total_calls: usize,
    pub(super) text_total_ns: usize,
    pub(super) wrap_calls: usize,
    pub(super) wrap_ns: usize,
}

thread_local! {
    static ACTIVE: RefCell<Option<TextMeasurementDiagnosticStats>> = const { RefCell::new(None) };
}

pub(super) struct TextMeasurementDiagnosticScope {
    finished: bool,
    previous: Option<TextMeasurementDiagnosticStats>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl TextMeasurementDiagnosticScope {
    pub(super) fn enter(enabled: bool) -> Self {
        let previous = ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            std::mem::replace(&mut *active, enabled.then_some(Default::default()))
        });
        Self {
            finished: false,
            previous,
            _not_send_sync: PhantomData,
        }
    }

    pub(super) fn finish(&mut self) -> TextMeasurementDiagnosticStats {
        if self.finished {
            return Default::default();
        }
        self.finished = true;
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            let current = std::mem::replace(&mut *active, self.previous.take());
            current.map_or(TextMeasurementDiagnosticStats::default(), |stats| stats)
        })
    }
}

impl Drop for TextMeasurementDiagnosticScope {
    fn drop(&mut self) {
        if !self.finished {
            ACTIVE.with(|active| {
                let mut active = active.borrow_mut();
                *active = self.previous.take();
            });
        }
    }
}

pub(super) fn start_measurement() -> Option<Instant> {
    ACTIVE.with(|active| active.borrow().is_some().then(Instant::now))
}

pub(super) fn record_lookup(hit: bool, measured_started: Option<Instant>) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(stats) = active.as_mut() else {
            return;
        };
        stats.lookups += 1;
        if hit {
            stats.hits += 1;
        } else {
            stats.misses += 1;
            if let Some(started) = measured_started {
                stats.measured_calls += 1;
                stats.measured_ns = stats.measured_ns.saturating_add(
                    started
                        .elapsed()
                        .as_nanos()
                        .try_into()
                        .unwrap_or(usize::MAX),
                );
            }
        }
    });
}

pub(super) fn finish_layout_svg_trace(
    trace: &HtmlDebugTrace,
    generation: u64,
    started: Option<Instant>,
    rendered: &LayoutResult,
    flow_before: (usize, usize),
    flow_after: (usize, usize),
    diagnostics: (TextMeasurementDiagnosticStats, SvgMeasurementStats),
) {
    let (text_stats, svg_stats) = diagnostics;
    let metrics =
        metrics::layout_svg_metrics(rendered, flow_before, flow_after, text_stats, svg_stats);
    trace.finish(generation, "layout_svg", started, &metrics);
}

pub(super) fn finish_layout_total_trace(
    trace: &HtmlDebugTrace,
    generation: u64,
    started: Option<Instant>,
    rendered: &LayoutResult,
    viewport: (u32, u32),
) {
    trace.finish(
        generation,
        "layout_total",
        started,
        &[
            ("svg_bytes", rendered.svg.len()),
            ("hit_targets", rendered.hit_targets.len()),
            ("element_boxes", rendered.element_boxes.len()),
            ("viewport_width", viewport.0 as usize),
            ("viewport_height", viewport.1 as usize),
        ],
    );
}

#[cfg(test)]
#[path = "text_measurement_diagnostic_tests.rs"]
mod tests;
