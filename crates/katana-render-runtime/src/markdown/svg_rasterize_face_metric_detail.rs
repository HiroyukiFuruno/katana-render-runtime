use super::ACTIVE;
use std::{marker::PhantomData, rc::Rc, time::Instant};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FaceMetricStats {
    pub(crate) acquire_ns: usize,
    pub(crate) tables_ns: usize,
    pub(crate) commit_ns: usize,
    pub(crate) legacy_ns: usize,
    pub(crate) acquire_calls: usize,
    pub(crate) tables_calls: usize,
    pub(crate) commit_calls: usize,
    pub(crate) legacy_calls: usize,
    pub(crate) bridge_some: usize,
    pub(crate) bridge_none: usize,
    pub(crate) bridge_err: usize,
}

#[derive(Clone, Copy)]
pub(in super::super) enum FaceMetricPhase {
    Acquire,
    Tables,
    Commit,
    Legacy,
}
#[derive(Clone, Copy)]
pub(in super::super) enum FaceMetricOutcome {
    Bridge,
    Legacy,
    Invalid,
}

pub(in super::super) struct FaceMetricScope {
    enabled: bool,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl FaceMetricScope {
    pub(in super::super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            _not_send_sync: PhantomData,
        }
    }

    pub(in super::super) fn measure<T>(
        &self,
        phase: FaceMetricPhase,
        body: impl FnOnce() -> T,
    ) -> T {
        let started = self.enabled.then(Instant::now);
        let result = body();
        if let Some(started) = started {
            let elapsed = started
                .elapsed()
                .as_nanos()
                .try_into()
                .unwrap_or(usize::MAX);
            ACTIVE.with(|active| {
                if let Some(stats) = active.borrow_mut().as_mut() {
                    record_phase(&mut stats.face_inner, phase, elapsed);
                }
            });
        }
        result
    }

    pub(in super::super) fn record_outcome(&self, outcome: FaceMetricOutcome) {
        if !self.enabled {
            return;
        }
        ACTIVE.with(|active| {
            if let Some(stats) = active.borrow_mut().as_mut() {
                let count = match outcome {
                    FaceMetricOutcome::Bridge => &mut stats.face_inner.bridge_some,
                    FaceMetricOutcome::Legacy => &mut stats.face_inner.bridge_none,
                    FaceMetricOutcome::Invalid => &mut stats.face_inner.bridge_err,
                };
                *count = count.saturating_add(1);
            }
        });
    }
}

fn record_phase(stats: &mut FaceMetricStats, phase: FaceMetricPhase, elapsed: usize) {
    let (duration, calls) = match phase {
        FaceMetricPhase::Acquire => (&mut stats.acquire_ns, &mut stats.acquire_calls),
        FaceMetricPhase::Tables => (&mut stats.tables_ns, &mut stats.tables_calls),
        FaceMetricPhase::Commit => (&mut stats.commit_ns, &mut stats.commit_calls),
        FaceMetricPhase::Legacy => (&mut stats.legacy_ns, &mut stats.legacy_calls),
    };
    *duration = duration.saturating_add(elapsed);
    *calls = calls.saturating_add(1);
}

#[cfg(test)]
mod tests {
    use super::super::Stats;

    const TEST_VALUE: usize = 1;
    use super::{FaceMetricOutcome, FaceMetricPhase, FaceMetricScope};

    #[test]
    fn disabled_face_detail_keeps_results_and_stats() {
        let mut outer = super::super::enter(false);
        let scope = FaceMetricScope::new(false);
        assert_eq!(
            scope.measure(FaceMetricPhase::Acquire, || Some(TEST_VALUE)),
            Some(TEST_VALUE)
        );
        scope.record_outcome(FaceMetricOutcome::Bridge);
        assert_eq!(outer.finish(), Stats::default());
    }

    #[test]
    fn face_detail_counts_are_scoped_without_timing_assumptions() {
        let mut outer = super::super::enter(true);
        let scope = FaceMetricScope::new(true);
        assert_eq!(
            scope.measure(FaceMetricPhase::Legacy, || Some(TEST_VALUE)),
            Some(TEST_VALUE)
        );
        scope.record_outcome(FaceMetricOutcome::Legacy);
        let stats = outer.finish();
        assert_eq!(stats.face_inner.legacy_calls, 1);
        assert_eq!(stats.face_inner.bridge_none, 1);
        assert_eq!(stats.face_inner.acquire_calls, 0);
    }

    #[test]
    fn invalid_face_outcomes_are_counted_only_during_the_measurement() {
        let mut outer = super::super::enter(true);
        let scope = FaceMetricScope::new(true);
        scope.record_outcome(FaceMetricOutcome::Invalid);
        let measured = outer.finish().face_inner;
        assert_eq!(measured.bridge_err, 1);
        assert_eq!(measured.bridge_some, 0);
        assert_eq!(measured.bridge_none, 0);

        assert_eq!(
            scope.measure(FaceMetricPhase::Tables, || TEST_VALUE),
            TEST_VALUE
        );
        scope.record_outcome(FaceMetricOutcome::Invalid);
        let mut next = super::super::enter(true);
        assert_eq!(next.finish(), Stats::default());
    }
}
