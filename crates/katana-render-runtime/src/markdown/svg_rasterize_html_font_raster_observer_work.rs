use super::counters::CacheState;
use sha2::{Digest, Sha256};

const SHA256_BYTES: usize = 32;
const STREAM_RECORD_COUNT: usize = 5;
const DOMAIN_COUNT: usize = 9;
const INPUT_RECORD_INDEX: usize = 0;
const FRAME_RECORD_INDEX: usize = 1;
const PARSE_RECORD_INDEX: usize = 2;
const ROUTE_RECORD_INDEX: usize = 3;
const SOURCE_OPEN_RECORD_INDEX: usize = 4;
const ACTION_DOMAIN_INDEX: usize = 0;
const OPEN_DOMAIN_INDEX: usize = 1;
const REFRESH_DOMAIN_INDEX: usize = 2;
const SCROLL_ITERATION_DOMAIN_INDEX: usize = 3;
const DOM_DOMAIN_INDEX: usize = 4;
const FRAME_DOMAIN_INDEX: usize = 5;
const PARSE_DOMAIN_INDEX: usize = 6;
const PARSE_RESULT_DOMAIN_INDEX: usize = 7;
const CLOSE_DOMAIN_INDEX: usize = 8;

#[derive(Clone, Default)]
pub(super) struct Work {
    inputs: Sha256,
    outputs: Sha256,
    parses: Sha256,
    routes: Sha256,
    source_opens: Sha256,
    pub(super) frame: u64,
    active_frame: u64,
    attempt_ordinal: u64,
    active_attempt: u64,
    counts: [u64; STREAM_RECORD_COUNT],
    domains: [u64; DOMAIN_COUNT],
    overflow: bool,
    endpoints: u64,
    bad_endpoints: u64,
    terminal: CacheState,
    opens: super::source_opens::OpenTracker,
}

fn add(count: &mut u64, overflow: &mut bool) {
    match count.checked_add(1) {
        Some(next) => *count = next,
        None => *overflow = true,
    }
}

fn feed(hash: &mut Sha256, domain: &str, bytes: &[u8]) {
    hash.update((domain.len() as u64).to_le_bytes());
    hash.update(domain.as_bytes());
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

pub(super) fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Work {
    pub(super) fn record(&mut self, domain: &str, bytes: &[u8]) {
        let (hash, index) = match domain {
            "frame" => (&mut self.outputs, FRAME_RECORD_INDEX),
            "parse" | "parse_result" => (&mut self.parses, PARSE_RECORD_INDEX),
            "route" => (&mut self.routes, ROUTE_RECORD_INDEX),
            "source_open" => (&mut self.source_opens, SOURCE_OPEN_RECORD_INDEX),
            _ => (&mut self.inputs, INPUT_RECORD_INDEX),
        };
        add(&mut self.counts[index], &mut self.overflow);
        let domain_index = match domain {
            "action" => Some(ACTION_DOMAIN_INDEX),
            "open" => Some(OPEN_DOMAIN_INDEX),
            "refresh" => Some(REFRESH_DOMAIN_INDEX),
            "scroll_iteration" => Some(SCROLL_ITERATION_DOMAIN_INDEX),
            "dom" => Some(DOM_DOMAIN_INDEX),
            "frame" => Some(FRAME_DOMAIN_INDEX),
            "parse" => Some(PARSE_DOMAIN_INDEX),
            "parse_result" => Some(PARSE_RESULT_DOMAIN_INDEX),
            "close" => Some(CLOSE_DOMAIN_INDEX),
            _ => None,
        };
        if let Some(index) = domain_index {
            add(&mut self.domains[index], &mut self.overflow);
        }
        feed(hash, domain, bytes);
    }

    pub(super) fn begin_frame(&mut self) -> u64 {
        let previous = self.active_frame;
        add(&mut self.frame, &mut self.overflow);
        self.active_frame = self.frame;
        previous
    }

    pub(super) fn begin_attempt(&mut self) -> u64 {
        let previous = self.active_attempt;
        add(&mut self.attempt_ordinal, &mut self.overflow);
        self.active_attempt = self.attempt_ordinal;
        previous
    }

    pub(super) fn end_attempt(&mut self, previous: u64) {
        self.active_attempt = previous;
    }

    pub(super) fn end_frame(&mut self, state: CacheState, previous: u64) {
        add(&mut self.endpoints, &mut self.overflow);
        if !state.released() {
            add(&mut self.bad_endpoints, &mut self.overflow);
        }
        self.terminal = state;
        self.active_frame = previous;
    }

    pub(super) fn source_open(&mut self, key: [u8; SHA256_BYTES]) {
        self.opens
            .observe(key, self.active_frame, self.active_attempt);
        let entry = serde_json::json!({"frame": self.active_frame,
            "attempt": self.active_attempt, "keySha256": hex(key)});
        self.record("source_open", entry.to_string().as_bytes());
    }

    pub(super) fn complete(&self) -> bool {
        !self.overflow
            && !self.opens.overflow
            && self.frame == self.endpoints
            && self.bad_endpoints == 0
            && self.active_attempt == 0
    }

    pub(super) fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "orderedInputSha256": hex(self.inputs.clone().finalize()),
            "orderedFrameOutputSha256": hex(self.outputs.clone().finalize()),
            "orderedParseSha256": hex(self.parses.clone().finalize()),
            "orderedRouteSha256": hex(self.routes.clone().finalize()),
            "orderedSourceOpenSha256": hex(self.source_opens.clone().finalize()),
            "streamRecordCounts": self.counts, "overflow": self.overflow,
            "actionDispatches": self.domains[0], "openCalls": self.domains[1],
            "refreshCalls": self.domains[2], "scrollLoopIterations": self.domains[3],
            "domProjections": self.domains[4], "storedFrames": self.domains[5],
            "parseAttempts": self.domains[6], "parseResults": self.domains[7], "closeCalls": self.domains[8],
            "attemptsStarted": self.attempt_ordinal, "terminalActiveAttempt": self.active_attempt,
            "frameAttempts": self.frame, "frameEndpoints": self.endpoints,
            "badFrameEndpoints": self.bad_endpoints, "terminalFrameCache": self.terminal,
            "sameFrameDifferentAttemptReopensObserved": self.opens.repeated,
            "reopenCounterOverflow": self.opens.overflow, "reopenTrackerSlots": 2, "reopenTrackerExhaustive": false,
            "timingAcceptance": false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{CacheState, Work};

    #[test]
    fn diagnostic_frame_count_overflow_rejects_an_incomplete_capture() {
        let mut work = Work {
            frame: u64::MAX - 1,
            endpoints: u64::MAX - 1,
            ..Work::default()
        };
        for (overflow, complete) in [(false, true), (true, false)] {
            let previous = work.begin_frame();
            work.end_frame(CacheState::default(), previous);
            let report = work.json();
            assert_eq!(report["frameAttempts"].as_u64(), Some(u64::MAX));
            assert_eq!(report["frameEndpoints"].as_u64(), Some(u64::MAX));
            assert_eq!(report["overflow"], overflow);
            assert_eq!(work.complete(), complete);
        }
    }

    #[test]
    fn diagnostic_frame_retaining_cache_invalidates_the_capture() {
        let mut work = Work::default();
        let open = CacheState {
            depth: 1,
            ..CacheState::default()
        };
        for (state, endpoints) in [(open, 1), (CacheState::default(), 2)] {
            let previous = work.begin_frame();
            work.end_frame(state, previous);
            let report = work.json();
            assert_eq!(report["frameEndpoints"].as_u64(), Some(endpoints));
            assert_eq!(report["badFrameEndpoints"].as_u64(), Some(1));
            assert!(!work.complete());
        }
    }
}
