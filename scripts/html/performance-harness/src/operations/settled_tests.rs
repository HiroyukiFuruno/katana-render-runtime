#![cfg(test)]

use super::super::super::test_support::settled_timer_evidence;
use super::super::CatalogOperations;
use crate::{options::CatalogOperationPlan, test_support::TestSupport};

fn malformed_timer_mutations() -> impl IntoIterator<Item = (&'static str, Option<serde_json::Value>)>
{
    [
        ("steps", Some(serde_json::json!([]))),
        ("state", None),
        ("state", Some(serde_json::Value::Null)),
        ("steps", None),
        ("steps", Some(serde_json::Value::Null)),
        ("steps", Some(serde_json::json!([serde_json::Value::Null]))),
    ]
}

fn apply_timer_mutation(
    evidence: &mut serde_json::Value,
    key: &str,
    replacement: Option<serde_json::Value>,
) -> bool {
    if let Some(value) = replacement {
        evidence[key] = value;
        true
    } else {
        evidence
            .as_object_mut()
            .and_then(|object| object.remove(key))
            .is_some()
    }
}

#[test]
fn settled_timer_rejects_missing_and_malformed_state_or_steps(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut operation_plan: CatalogOperationPlan =
        serde_json::from_value(TestSupport::catalog_test_plan())?;
    operation_plan.operation_kind = "script-await-timer-microtask".to_owned();
    operation_plan.operation_stream.clear();
    let valid_evidence = settled_timer_evidence();
    assert!(CatalogOperations::is_settled_timer(
        &operation_plan,
        &valid_evidence
    ));
    for (key, replacement) in malformed_timer_mutations() {
        let mut evidence = valid_evidence.clone();
        assert!(apply_timer_mutation(&mut evidence, key, replacement));
        assert!(!CatalogOperations::is_settled_timer(
            &operation_plan,
            &evidence
        ));
    }
    Ok(())
}
