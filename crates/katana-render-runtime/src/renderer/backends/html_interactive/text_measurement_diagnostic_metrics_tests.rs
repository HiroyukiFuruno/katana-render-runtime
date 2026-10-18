use super::micros;

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
