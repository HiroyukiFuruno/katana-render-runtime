#![cfg(test)]

use super::catalog_condition;
use crate::{options::OperationPlan, test_support::TestSupport};

#[test]
fn catalog_condition_preserves_legacy_null_contract() -> Result<(), Box<dyn std::error::Error>> {
    let plan = serde_json::from_value::<OperationPlan>(serde_json::json!({
        "fixture_sha256": "legacy",
        "operation": "click",
        "x": 1.0,
        "y": 2.0,
        "expected_dom_state_path": "state.json",
        "expected_dom_state_sha256": "state"
    }))?;
    assert_eq!(catalog_condition(&plan), serde_json::Value::Null);
    Ok(())
}

#[test]
fn catalog_condition_preserves_catalog_identity_and_pins() -> Result<(), Box<dyn std::error::Error>>
{
    let plan = serde_json::from_value::<OperationPlan>(TestSupport::catalog_test_plan())?;
    let condition = catalog_condition(&plan);
    assert_eq!(condition["profile"], "catalog-plan84");
    assert_eq!(condition["condition_id"], "async/expected-state/1280x900@1");
    assert_eq!(condition["case_id"], "async");
    assert_eq!(condition["state"], "expected-state");
    assert_eq!(condition["operation_kind"], "script-await-timer-microtask");
    assert_eq!(condition["target_selector"], "#state");
    assert_eq!(condition["operation_evidence_path"], "operation.json");
    assert_eq!(condition["operation_evidence_sha256"], "b".repeat(64));
    assert_eq!(condition["evidence_binary_sha256"], "c".repeat(64));
    Ok(())
}
