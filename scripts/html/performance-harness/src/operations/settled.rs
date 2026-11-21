use serde_json::{Map, Value};

type SettledStateValidator = fn(&Map<String, Value>, &[Value]) -> bool;

pub(super) fn settled_timer_evidence_is_valid(evidence: &Value, operation_kind: &str) -> bool {
    settled_evidence_is_valid(evidence, operation_kind, settled_timer_state_is_valid)
}

fn settled_timer_state_is_valid(state: &Map<String, Value>, steps: &[Value]) -> bool {
    let Some(step) = steps.first() else {
        return false;
    };
    state.len() == 2
        && has_exact_keys(state, &["before", "after"])
        && state.get("before") == state.get("after")
        && steps.len() == 1
        && settled_timer_step_is_valid(step, state)
}

fn settled_evidence_is_valid(
    evidence: &Value,
    operation_kind: &str,
    state_validator: SettledStateValidator,
) -> bool {
    let Some(object) = evidence.as_object() else {
        return false;
    };
    if !settled_identity_is_valid(object, operation_kind) {
        return false;
    }
    let Some(state) = object.get("state").and_then(Value::as_object) else {
        return false;
    };
    let Some(steps) = object.get("steps").and_then(Value::as_array) else {
        return false;
    };
    state_validator(state, steps)
}

fn settled_identity_is_valid(object: &Map<String, Value>, operation_kind: &str) -> bool {
    has_exact_keys(
        object,
        &[
            "schema",
            "kind",
            "status",
            "sourceSha256Before",
            "sourceSha256After",
            "preFrame",
            "postFrame",
            "target",
            "state",
            "steps",
            "delivery",
        ],
    ) && object.get("kind").and_then(Value::as_str) == Some(operation_kind)
        && object.get("status").and_then(Value::as_str) == Some("observed")
        && object.get("delivery").and_then(Value::as_str) == Some("session-load-settled")
}

fn settled_timer_step_is_valid(value: &Value, state: &Map<String, Value>) -> bool {
    let Some(step) = value.as_object() else {
        return false;
    };
    has_exact_keys(
        step,
        &[
            "kind",
            "elapsedMs",
            "timerRan",
            "microtaskCheckpoint",
            "stateBefore",
            "stateAfter",
            "fixtureStateVerified",
        ],
    ) && step.get("kind").and_then(Value::as_str) == Some("host-time-advance")
        && step.get("elapsedMs").and_then(Value::as_f64) == Some(0.0)
        && step.get("timerRan").and_then(Value::as_bool) == Some(false)
        && step.get("microtaskCheckpoint").and_then(Value::as_str) == Some("runtime-advance-timers")
        && step.get("fixtureStateVerified").and_then(Value::as_bool) == Some(true)
        && step.get("stateBefore") == state.get("before")
        && step.get("stateAfter") == state.get("after")
}

pub(super) fn settled_resource_evidence_is_valid(evidence: &Value, operation_kind: &str) -> bool {
    settled_evidence_is_valid(evidence, operation_kind, settled_resource_state_is_valid)
}

fn settled_resource_state_is_valid(state: &Map<String, Value>, steps: &[Value]) -> bool {
    let Some(step) = steps.first().and_then(Value::as_object) else {
        return false;
    };
    state.len() == 2
        && has_exact_keys(state, &["before", "after"])
        && state.get("before") == state.get("after")
        && steps.len() == 1
        && has_exact_keys(step, &["kind", "stateBefore", "stateAfter"])
        && step.get("kind").and_then(Value::as_str) == Some("session-load-settled-resource-state")
        && step.get("stateBefore") == state.get("before")
        && step.get("stateAfter") == state.get("after")
}

fn has_exact_keys(object: &Map<String, Value>, keys: &[&str]) -> bool {
    object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
}

#[cfg(test)]
#[path = "settled_tests.rs"]
mod tests;
