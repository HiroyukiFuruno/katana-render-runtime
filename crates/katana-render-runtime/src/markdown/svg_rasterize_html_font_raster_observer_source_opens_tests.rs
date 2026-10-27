use super::OpenTracker;

struct Observation {
    key: u8,
    frame: u64,
    attempt: u64,
    repeated: u64,
}

const REOPEN_CASES: &[Observation] = &[
    Observation {
        key: 1,
        frame: 7,
        attempt: 2,
        repeated: 0,
    },
    Observation {
        key: 1,
        frame: 7,
        attempt: 2,
        repeated: 0,
    },
    Observation {
        key: 1,
        frame: 7,
        attempt: 3,
        repeated: 1,
    },
    Observation {
        key: 1,
        frame: 8,
        attempt: 1,
        repeated: 1,
    },
    Observation {
        key: 1,
        frame: 8,
        attempt: 2,
        repeated: 2,
    },
    Observation {
        key: 2,
        frame: 8,
        attempt: 2,
        repeated: 2,
    },
    Observation {
        key: 2,
        frame: 8,
        attempt: 4,
        repeated: 3,
    },
];

fn source_key(value: u8) -> [u8; super::SHA256_BYTES] {
    [value; super::SHA256_BYTES]
}

#[test]
fn repeated_opens_count_only_same_source_and_frame_with_a_new_attempt() {
    let mut tracker = OpenTracker::default();

    for observation in REOPEN_CASES {
        tracker.observe(
            source_key(observation.key),
            observation.frame,
            observation.attempt,
        );
        assert_eq!(tracker.repeated, observation.repeated);
        assert!(!tracker.overflow);
    }
}

#[test]
fn zero_frame_or_attempt_does_not_create_or_replace_a_tracked_open() {
    let mut tracker = OpenTracker::default();

    tracker.observe(source_key(1), 0, 1);
    tracker.observe(source_key(1), 1, 0);
    assert_eq!(tracker.repeated, 0);

    tracker.observe(source_key(1), 1, 1);
    tracker.observe(source_key(1), 1, 2);

    assert_eq!(tracker.repeated, 1);
    assert!(!tracker.overflow);
}

#[test]
fn two_slots_roll_over_without_counting_an_evicted_source_as_repeated() {
    let mut tracker = OpenTracker::default();

    tracker.observe(source_key(1), 4, 1);
    tracker.observe(source_key(2), 4, 1);
    tracker.observe(source_key(3), 4, 1);
    tracker.observe(source_key(2), 4, 2);
    assert_eq!(tracker.repeated, 1);

    tracker.observe(source_key(4), 4, 1);
    tracker.observe(source_key(2), 4, 3);

    assert_eq!(tracker.repeated, 1);
    assert!(!tracker.overflow);
}

#[test]
fn repeated_counter_overflow_is_reported_without_wrapping() {
    let mut tracker = OpenTracker::default();
    tracker.observe(source_key(1), 5, 1);
    tracker.repeated = u64::MAX;

    /* WHY: 実行回数で到達できない境界を状態遷移として固定する。 */
    tracker.observe(source_key(1), 5, 2);

    assert_eq!(tracker.repeated, u64::MAX);
    assert!(tracker.overflow);
}
