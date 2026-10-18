use super::super::super::document::wrap_text_with_style;
use super::super::super::style::CssStyle;
use super::super::super::text_metrics::text_width;
use super::super::{
    TextMeasurementDiagnosticScope, TextMeasurementDiagnosticStats, record_text_total_timing,
    start_text_total_timing,
};

const MIXED_TEXT: &str = "日本語 ABC";
const WIDTH_EPSILON: f32 = 0.5;
const NARROW_WIDTH: f32 = 40.0;
const EXPECTED_LOOKUPS: usize = 2;
const EXPECTED_HITS: usize = 1;
const EXPECTED_MISSES: usize = 1;
const EXPECTED_WRAP_CALLS: usize = 1;
const EXPECTED_SINGLE_CALL: usize = 1;
const NO_CALLS: usize = 0;

#[test]
fn real_text_width_records_total_calls_and_cache_hit() {
    let style = CssStyle::browser_default();
    let _cache_scope = super::super::super::text_metrics::TextMeasurementCacheScope::enter();
    let mut scope = TextMeasurementDiagnosticScope::enter(true);
    assert!(super::super::start_measurement().is_some());
    let first = text_width(MIXED_TEXT, &style);
    let second = text_width(MIXED_TEXT, &style);
    let stats = scope.finish();

    assert_eq!(first, second);
    assert_eq!(stats.text_total_calls, EXPECTED_LOOKUPS);
    assert_eq!(stats.lookups, EXPECTED_LOOKUPS);
    assert_eq!(stats.hits, EXPECTED_HITS);
    assert_eq!(stats.misses, EXPECTED_MISSES);
}

#[test]
fn real_wrap_records_one_call_and_exact_mixed_lines() {
    let style = CssStyle::browser_default();
    let width = text_width("日本語", &style) + WIDTH_EPSILON;
    let mut scope = TextMeasurementDiagnosticScope::enter(true);
    let lines = wrap_text_with_style(MIXED_TEXT, width, &style);
    let stats = scope.finish();

    assert_eq!(lines, ["日本語", "ABC"]);
    assert_eq!(stats.wrap_calls, EXPECTED_WRAP_CALLS);
    assert!(stats.text_total_calls > NO_CALLS);
}

#[test]
fn disabled_and_nested_scopes_track_only_real_calls() {
    let style = CssStyle::browser_default();
    let mut outer = TextMeasurementDiagnosticScope::enter(true);
    let _ = text_width("outer", &style);
    let mut nested = TextMeasurementDiagnosticScope::enter(true);
    let _ = wrap_text_with_style(MIXED_TEXT, NARROW_WIDTH, &style);

    let mut disabled = TextMeasurementDiagnosticScope::enter(false);
    let _ = text_width("disabled", &style);
    assert_eq!(disabled.finish(), TextMeasurementDiagnosticStats::default());
    let nested_stats = nested.finish();
    assert_eq!(nested_stats.wrap_calls, EXPECTED_WRAP_CALLS);
    assert!(nested_stats.text_total_calls > NO_CALLS);
    let outer_stats = outer.finish();
    assert_eq!(outer_stats.text_total_calls, EXPECTED_SINGLE_CALL);
    assert_eq!(outer_stats.wrap_calls, NO_CALLS);
}

#[test]
fn real_calls_restore_scope_after_unwind() {
    let style = CssStyle::browser_default();
    let _ = std::panic::catch_unwind(|| {
        let _scope = TextMeasurementDiagnosticScope::enter(true);
        let _ = text_width("unwind", &style);
        let _ = wrap_text_with_style(MIXED_TEXT, NARROW_WIDTH, &style);
        std::panic::resume_unwind(Box::new("flow diagnostic unwind"));
    });

    assert!(super::super::start_measurement().is_none());
    let mut restored = TextMeasurementDiagnosticScope::enter(true);
    let _ = text_width("restored", &style);
    let stats = restored.finish();
    assert_eq!(stats.text_total_calls, EXPECTED_SINGLE_CALL);
    assert_eq!(stats.wrap_calls, NO_CALLS);
}

#[test]
fn finished_timer_record_does_not_leak_into_the_next_scope() {
    let mut finished = TextMeasurementDiagnosticScope::enter(true);
    let started = start_text_total_timing();
    assert!(started.is_some());
    let _ = finished.finish();
    record_text_total_timing(started);

    let style = CssStyle::browser_default();
    let mut next = TextMeasurementDiagnosticScope::enter(true);
    let _ = text_width("lifecycle", &style);
    let stats = next.finish();
    assert_eq!(stats.text_total_calls, EXPECTED_SINGLE_CALL);
}
