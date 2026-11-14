use super::{TextMeasurementDiagnosticStats, micros, timing_metrics};

const SUB_MICROSECOND_INTERVAL_NS: usize = 600;
const FIRST_INTERVAL: usize = 0;
const INTERVAL_COUNT: usize = 3;
const EXPECTED_SUM_NS: usize = SUB_MICROSECOND_INTERVAL_NS * INTERVAL_COUNT;
const EXPECTED_MICROSECONDS: usize = 1;
const OLD_TRUNCATED_RESULT: usize = 0;

#[test]
fn micros_converts_aggregated_sub_microsecond_intervals_once() {
    let interval_sum = (FIRST_INTERVAL..INTERVAL_COUNT)
        .map(|_| SUB_MICROSECOND_INTERVAL_NS)
        .sum::<usize>();

    assert_eq!(interval_sum, EXPECTED_SUM_NS);
    assert_eq!(micros(interval_sum), EXPECTED_MICROSECONDS);
    assert_ne!(micros(interval_sum), OLD_TRUNCATED_RESULT);
}

#[test]
fn timing_metrics_preserves_existing_keys_and_reports_preparation_once() {
    let stats = TextMeasurementDiagnosticStats {
        preparation_calls: 1,
        preparation_ns: EXPECTED_MICROSECONDS * 1_000,
        wrap_calls: INTERVAL_COUNT,
        ..Default::default()
    };
    let metrics = timing_metrics(stats);
    assert_eq!(
        metrics
            .iter()
            .filter(|(key, _)| *key == "wrap_calls")
            .count(),
        1
    );
    assert_eq!(
        metrics
            .iter()
            .find(|(key, _)| *key == "text_width_preparation_calls")
            .map(|(_, value)| *value),
        Some(1)
    );
    assert_eq!(
        metrics
            .iter()
            .find(|(key, _)| *key == "text_width_preparation_us")
            .map(|(_, value)| *value),
        Some(EXPECTED_MICROSECONDS)
    );
}
