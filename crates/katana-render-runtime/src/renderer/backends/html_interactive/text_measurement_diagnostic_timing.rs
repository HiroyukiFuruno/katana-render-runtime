use super::{ACTIVE, TextMeasurementDiagnosticStats};
use std::time::Instant;

pub(in crate::renderer::backends::html_interactive) fn start_style_timing() -> Option<Instant> {
    ACTIVE.with(|active| active.borrow().is_some().then(Instant::now))
}

pub(in crate::renderer::backends::html_interactive) fn record_style_timing(
    started: Option<Instant>,
) {
    record_timing(started, |stats| {
        (&mut stats.style_calls, &mut stats.style_ns)
    });
}

pub(in crate::renderer::backends::html_interactive) fn start_taffy_timing() -> Option<Instant> {
    ACTIVE.with(|active| active.borrow().is_some().then(Instant::now))
}

pub(in crate::renderer::backends::html_interactive) fn record_taffy_timing(
    started: Option<Instant>,
) {
    record_timing(started, |stats| {
        (&mut stats.taffy_calls, &mut stats.taffy_ns)
    });
}

pub(in crate::renderer::backends::html_interactive) fn start_text_total_timing() -> Option<Instant>
{
    ACTIVE.with(|active| active.borrow().is_some().then(Instant::now))
}

pub(in crate::renderer::backends::html_interactive) fn record_text_total_timing(
    started: Option<Instant>,
) {
    record_timing(started, |stats| {
        (&mut stats.text_total_calls, &mut stats.text_total_ns)
    });
}

pub(in crate::renderer::backends::html_interactive) fn start_wrap_timing() -> Option<Instant> {
    ACTIVE.with(|active| active.borrow().is_some().then(Instant::now))
}

pub(in crate::renderer::backends::html_interactive) fn record_wrap_timing(
    started: Option<Instant>,
) {
    record_timing(started, |stats| (&mut stats.wrap_calls, &mut stats.wrap_ns));
}

fn record_timing(
    started: Option<Instant>,
    fields: impl FnOnce(&mut TextMeasurementDiagnosticStats) -> (&mut usize, &mut usize),
) {
    let Some(started) = started else {
        return;
    };
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(stats) = active.as_mut() else {
            return;
        };
        let (calls, elapsed_ns) = fields(stats);
        *calls += 1;
        let elapsed = started
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(usize::MAX);
        *elapsed_ns = (*elapsed_ns).saturating_add(elapsed);
    });
}
