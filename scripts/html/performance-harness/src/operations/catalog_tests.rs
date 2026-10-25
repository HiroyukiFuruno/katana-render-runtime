#![cfg(test)]

use super::CatalogOperations;
use crate::{options::CatalogOperationPlan, test_support::TestSupport};

fn settled_plan() -> Result<CatalogOperationPlan, Box<dyn std::error::Error>> {
    Ok(serde_json::from_value(TestSupport::catalog_test_plan())?)
}

fn settled_evidence() -> serde_json::Value {
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

#[test]
fn settled_timer_accepts_nested_operation_evidence() -> Result<(), Box<dyn std::error::Error>> {
    assert!(CatalogOperations::is_settled_timer(
        &settled_plan()?,
        &settled_evidence()
    ));
    Ok(())
}

#[test]
fn settled_timer_rejects_tampered_nested_operation_evidence(
) -> Result<(), Box<dyn std::error::Error>> {
    let plan = settled_plan()?;
    let mut evidence = settled_evidence();
    evidence["steps"][0]["timerRan"] = serde_json::Value::Bool(true);
    assert!(!CatalogOperations::is_settled_timer(&plan, &evidence));
    Ok(())
}

#[test]
fn settled_resource_rejects_missing_resource_settlement_step(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut plan = settled_plan()?;
    plan.operation_kind = "script-await-resource-font".to_owned();
    let mut evidence = settled_evidence();
    evidence["kind"] = serde_json::Value::String("script-await-resource-font".to_owned());
    evidence["steps"][0]["kind"] = serde_json::Value::String("host-time-advance".to_owned());
    assert!(!CatalogOperations::is_settled_resource(&plan, &evidence));
    Ok(())
}

fn settled_resource_evidence() -> serde_json::Value {
    let mut evidence = settled_evidence();
    evidence["kind"] = serde_json::Value::String("script-await-resource-font".to_owned());
    evidence["steps"] = serde_json::json!([{
        "kind": "session-load-settled-resource-state",
        "stateBefore": "microtask>ready>timer",
        "stateAfter": "microtask>ready>timer",
    }]);
    evidence
}

fn remove_key(evidence: &mut serde_json::Value, key: &str) -> bool {
    evidence
        .as_object_mut()
        .and_then(|object| object.remove(key))
        .is_some()
}

fn replace_key(evidence: &mut serde_json::Value, key: &str, value: serde_json::Value) -> bool {
    if let Some(object) = evidence.as_object_mut() {
        object.insert(key.to_owned(), value);
        true
    } else {
        false
    }
}

fn mutate_step(evidence: &mut serde_json::Value, key: &str, value: serde_json::Value) -> bool {
    evidence
        .get_mut("steps")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|steps| steps.first_mut())
        .and_then(serde_json::Value::as_object_mut)
        .map(|step| step.insert(key.to_owned(), value).is_some())
        .is_some()
}

fn remove_step_key(evidence: &mut serde_json::Value, key: &str) -> bool {
    evidence
        .get_mut("steps")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|steps| steps.first_mut())
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|step| step.remove(key))
        .is_some()
}

fn mutate_timer_case(evidence: &mut serde_json::Value, name: &str) -> bool {
    match name {
        "non-object" => {
            *evidence = serde_json::Value::Null;
            true
        }
        "missing-state" => remove_key(evidence, "state"),
        "missing-steps" => remove_key(evidence, "steps"),
        "wrong-kind" => replace_key(evidence, "kind", serde_json::json!("other")),
        "wrong-status" => replace_key(evidence, "status", serde_json::json!("captured")),
        "wrong-delivery" => replace_key(evidence, "delivery", serde_json::json!("other")),
        "extra-key" => replace_key(evidence, "extra", serde_json::json!(true)),
        _ => false,
    }
}

fn mutate_resource_case(evidence: &mut serde_json::Value, name: &str) -> bool {
    match name {
        "empty-steps" => {
            evidence["steps"] = serde_json::json!([]);
            true
        }
        "wrong-step-kind" => mutate_step(evidence, "kind", serde_json::json!("other")),
        "missing-before" => remove_step_key(evidence, "stateBefore"),
        "wrong-after" => mutate_step(evidence, "stateAfter", serde_json::json!("other")),
        _ => false,
    }
}

fn assert_timer_mutations(plan: &CatalogOperationPlan) -> Result<(), String> {
    for name in [
        "non-object",
        "missing-state",
        "missing-steps",
        "wrong-kind",
        "wrong-status",
        "wrong-delivery",
        "extra-key",
    ] {
        let mut evidence = settled_evidence();
        if !mutate_timer_case(&mut evidence, name) {
            return Err(format!("timer mutation failed: {name}"));
        }
        if CatalogOperations::is_settled_timer(plan, &evidence) {
            return Err(format!("timer mutation accepted: {name}"));
        }
    }
    Ok(())
}

fn assert_resource_mutations(plan: &CatalogOperationPlan) -> Result<(), String> {
    let valid = settled_resource_evidence();
    if !CatalogOperations::is_settled_resource(plan, &valid) {
        return Err("resource positive control rejected".to_owned());
    }
    for name in [
        "empty-steps",
        "wrong-step-kind",
        "missing-before",
        "wrong-after",
    ] {
        let mut evidence = valid.clone();
        if !mutate_resource_case(&mut evidence, name) {
            return Err(format!("resource mutation failed: {name}"));
        }
        if CatalogOperations::is_settled_resource(plan, &evidence) {
            return Err(format!("resource mutation accepted: {name}"));
        }
    }
    Ok(())
}

#[test]
fn settled_evidence_rejects_structural_identity_and_resource_mutations(
) -> Result<(), Box<dyn std::error::Error>> {
    let timer_plan = settled_plan()?;
    assert_timer_mutations(&timer_plan)?;
    let mut resource_plan = timer_plan;
    resource_plan.operation_kind = "script-await-resource-font".to_owned();
    assert_resource_mutations(&resource_plan)?;
    Ok(())
}
