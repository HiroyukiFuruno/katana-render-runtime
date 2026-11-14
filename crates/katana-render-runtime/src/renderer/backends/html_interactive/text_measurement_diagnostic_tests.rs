use super::{TextMeasurementDiagnosticScope, TextMeasurementDiagnosticStats};

#[test]
fn disabled_scope_has_no_measurement_state() {
    let mut scope = TextMeasurementDiagnosticScope::enter(false);
    assert!(super::start_measurement().is_none());
    assert_eq!(scope.finish(), TextMeasurementDiagnosticStats::default());
}

#[test]
fn real_text_duplicate_records_lookup_and_cache_hit() {
    let style = super::super::style::CssStyle::browser_default();
    let _cache_scope = super::super::text_metrics::TextMeasurementCacheScope::enter();
    let mut scope = TextMeasurementDiagnosticScope::enter(true);
    assert!(super::start_measurement().is_some());
    let first = super::super::text_metrics::text_width("diagnostic text", &style);
    let second = super::super::text_metrics::text_width("diagnostic text", &style);
    let stats = scope.finish();
    assert_eq!(first, second);
    assert_eq!(stats.lookups, 2);
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.measured_calls, 1);
}

#[test]
fn nested_scope_restores_outer_state() {
    let mut outer = TextMeasurementDiagnosticScope::enter(true);
    super::record_lookup(false, None);
    let mut nested = TextMeasurementDiagnosticScope::enter(true);
    super::record_lookup(true, None);
    assert_eq!(nested.finish().hits, 1);
    super::record_lookup(true, None);
    let stats = outer.finish();
    assert_eq!(
        stats,
        TextMeasurementDiagnosticStats {
            lookups: 2,
            misses: 1,
            hits: 1,
            ..Default::default()
        }
    );
}

#[test]
fn disabled_nested_scope_suppresses_and_restores_outer_state() {
    let mut outer = TextMeasurementDiagnosticScope::enter(true);
    super::record_lookup(true, None);
    let mut disabled = TextMeasurementDiagnosticScope::enter(false);
    super::record_lookup(false, None);
    assert_eq!(disabled.finish(), TextMeasurementDiagnosticStats::default());
    super::record_lookup(false, None);
    let stats = outer.finish();
    assert_eq!(stats.lookups, 2);
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
}

#[test]
fn repeated_finish_does_not_consume_restored_state() {
    let mut outer = TextMeasurementDiagnosticScope::enter(true);
    super::record_lookup(true, None);
    let mut inner = TextMeasurementDiagnosticScope::enter(true);
    super::record_lookup(false, None);
    assert_eq!(inner.finish().lookups, 1);
    assert_eq!(inner.finish(), TextMeasurementDiagnosticStats::default());
    super::record_lookup(true, None);
    let stats = outer.finish();
    assert_eq!(stats.lookups, 2);
    assert_eq!(stats.hits, 2);
}

#[test]
fn dropped_scope_cleans_up_after_unwind() {
    let _ = std::panic::catch_unwind(|| {
        let _scope = TextMeasurementDiagnosticScope::enter(true);
        super::record_lookup(false, None);
        std::panic::resume_unwind(Box::new("diagnostic unwind"));
    });
    assert!(super::start_measurement().is_none());
    let mut outer = TextMeasurementDiagnosticScope::enter(true);
    super::record_lookup(true, None);
    let _ = std::panic::catch_unwind(|| {
        let _inner = TextMeasurementDiagnosticScope::enter(true);
        super::record_lookup(false, None);
        std::panic::resume_unwind(Box::new("nested diagnostic unwind"));
    });
    super::record_lookup(true, None);
    let restored = outer.finish();
    assert_eq!(restored.lookups, 2);
    assert_eq!(restored.hits, 2);
    assert_eq!(restored.misses, 0);
}

#[path = "text_measurement_layout_diagnostic_tests.rs"]
mod layout_timing;

#[path = "text_measurement_flow_diagnostic_tests.rs"]
mod flow_timing;
