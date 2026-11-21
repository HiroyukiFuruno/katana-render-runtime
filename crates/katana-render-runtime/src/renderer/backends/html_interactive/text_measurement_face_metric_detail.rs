use super::{SvgMeasurementStats, micros};

const FACE_PHASE_COUNT: usize = 4;
const FACE_OUTCOME_COUNT: usize = 3;
pub(super) const FACE_DETAIL_COUNT: usize = FACE_PHASE_COUNT * 2 + FACE_OUTCOME_COUNT;

pub(super) fn face_detail_metrics(
    stats: SvgMeasurementStats,
) -> [(&'static str, usize); FACE_DETAIL_COUNT] {
    let timing = timing_metrics(stats);
    let calls = call_metrics(stats);
    let outcomes = outcome_metrics(stats);
    std::array::from_fn(|index| {
        if index < FACE_PHASE_COUNT {
            timing[index]
        } else if index < FACE_PHASE_COUNT * 2 {
            calls[index - FACE_PHASE_COUNT]
        } else {
            outcomes[index - FACE_PHASE_COUNT * 2]
        }
    })
}

fn timing_metrics(stats: SvgMeasurementStats) -> [(&'static str, usize); FACE_PHASE_COUNT] {
    [
        ("line_face_acquire_us", micros(stats.face_inner.acquire_ns)),
        ("line_face_tables_us", micros(stats.face_inner.tables_ns)),
        ("line_face_commit_us", micros(stats.face_inner.commit_ns)),
        ("line_face_legacy_us", micros(stats.face_inner.legacy_ns)),
    ]
}

fn call_metrics(stats: SvgMeasurementStats) -> [(&'static str, usize); FACE_PHASE_COUNT] {
    [
        ("line_face_acquire_calls", stats.face_inner.acquire_calls),
        ("line_face_tables_calls", stats.face_inner.tables_calls),
        ("line_face_commit_calls", stats.face_inner.commit_calls),
        ("line_face_legacy_calls", stats.face_inner.legacy_calls),
    ]
}

fn outcome_metrics(stats: SvgMeasurementStats) -> [(&'static str, usize); FACE_OUTCOME_COUNT] {
    [
        ("line_face_bridge_some", stats.face_inner.bridge_some),
        ("line_face_bridge_none", stats.face_inner.bridge_none),
        ("line_face_bridge_err", stats.face_inner.bridge_err),
    ]
}
