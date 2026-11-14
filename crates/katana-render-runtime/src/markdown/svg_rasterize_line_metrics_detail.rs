use super::{ACTIVE, Stats};
use std::{marker::PhantomData, rc::Rc, time::Instant};

#[derive(Clone, Copy)]
enum LineMetricsDetailPhase {
    Database,
    Matching,
    Runs,
    Faces,
}

#[derive(Default)]
pub(in super::super) struct LineMetricsDetailScope {
    stats: Stats,
    active_phase: Option<(LineMetricsDetailPhase, Instant)>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl LineMetricsDetailScope {
    pub(in super::super) fn start_database(&mut self) {
        self.start_phase(LineMetricsDetailPhase::Database);
    }

    pub(in super::super) fn start_matching(&mut self) {
        self.start_phase(LineMetricsDetailPhase::Matching);
    }

    pub(in super::super) fn start_runs(&mut self) {
        self.start_phase(LineMetricsDetailPhase::Runs);
    }

    pub(in super::super) fn start_faces(&mut self) {
        self.start_phase(LineMetricsDetailPhase::Faces);
    }

    pub(in super::super) fn finish_phase(&mut self) {
        let Some((phase, started)) = self.active_phase.take() else {
            return;
        };
        let elapsed = started
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(usize::MAX);
        let total = match phase {
            LineMetricsDetailPhase::Database => &mut self.stats.line_metric_db_ns,
            LineMetricsDetailPhase::Matching => &mut self.stats.line_metric_matching_ns,
            LineMetricsDetailPhase::Runs => &mut self.stats.line_metric_runs_ns,
            LineMetricsDetailPhase::Faces => &mut self.stats.line_metric_faces_ns,
        };
        *total = total.saturating_add(elapsed);
    }

    pub(in super::super) fn record_face_source(&mut self, source: &resvg::usvg::fontdb::Source) {
        let calls = match source {
            resvg::usvg::fontdb::Source::File(_) => &mut self.stats.line_metric_face_file_calls,
            resvg::usvg::fontdb::Source::Binary(_) => &mut self.stats.line_metric_face_binary_calls,
            resvg::usvg::fontdb::Source::SharedFile(_, _) => {
                &mut self.stats.line_metric_face_shared_calls
            }
        };
        *calls = calls.saturating_add(1);
    }

    fn start_phase(&mut self, phase: LineMetricsDetailPhase) {
        self.finish_phase();
        self.active_phase = Some((phase, Instant::now()));
    }
}

impl Drop for LineMetricsDetailScope {
    fn drop(&mut self) {
        self.finish_phase();
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            let Some(stats) = active.as_mut() else {
                return;
            };
            stats.line_metric_db_ns = stats
                .line_metric_db_ns
                .saturating_add(self.stats.line_metric_db_ns);
            stats.line_metric_matching_ns = stats
                .line_metric_matching_ns
                .saturating_add(self.stats.line_metric_matching_ns);
            stats.line_metric_runs_ns = stats
                .line_metric_runs_ns
                .saturating_add(self.stats.line_metric_runs_ns);
            stats.line_metric_faces_ns = stats
                .line_metric_faces_ns
                .saturating_add(self.stats.line_metric_faces_ns);
            stats.line_metric_face_file_calls = stats
                .line_metric_face_file_calls
                .saturating_add(self.stats.line_metric_face_file_calls);
            stats.line_metric_face_binary_calls = stats
                .line_metric_face_binary_calls
                .saturating_add(self.stats.line_metric_face_binary_calls);
            stats.line_metric_face_shared_calls = stats
                .line_metric_face_shared_calls
                .saturating_add(self.stats.line_metric_face_shared_calls);
        });
    }
}
