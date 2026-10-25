#![cfg(test)]

pub(super) fn settled_timer_evidence() -> serde_json::Value {
    serde_json::json!({
        "schema": "krr-html84-operation-evidence-v1",
        "kind": "script-await-timer-microtask",
        "status": "observed",
        "sourceSha256Before": "a".repeat(64),
        "sourceSha256After": "a".repeat(64),
        "preFrame": {"generation": 1, "scrollY": 0},
        "postFrame": {"generation": 1, "scrollY": 0},
        "target": {"visible": true, "nodeId": 7, "selector": "#state"},
        "state": {"before": "microtask>ready>timer", "after": "microtask>ready>timer"},
        "steps": [{
            "kind": "host-time-advance", "elapsedMs": 0, "timerRan": false,
            "microtaskCheckpoint": "runtime-advance-timers",
            "stateBefore": "microtask>ready>timer", "stateAfter": "microtask>ready>timer",
            "fixtureStateVerified": true,
        }],
        "delivery": "session-load-settled",
    })
}
