use super::super::options::CatalogOperationPlan;
use super::super::HtmlBrowserSession;

#[path = "dispatch.rs"]
mod dispatch;
#[path = "settled.rs"]
mod settled;
use settled::{settled_resource_evidence_is_valid, settled_timer_evidence_is_valid};

pub(crate) struct CatalogOperations;

impl CatalogOperations {
    pub(crate) fn is_static(plan: &CatalogOperationPlan) -> bool {
        matches!(
            plan.operation_kind.as_str(),
            "load-and-observe-target"
                | "input-state-empty"
                | "input-state-disabled-observe"
                | "script-load-observe"
                | "script-await-mutation-delivery"
        )
    }

    pub(crate) fn is_settled_timer(
        plan: &CatalogOperationPlan,
        evidence: &serde_json::Value,
    ) -> bool {
        if !matches!(plan.operation_kind.as_str(), "script-await-timer-microtask")
            || !plan.operation_stream.is_empty()
        {
            return false;
        }
        settled_timer_evidence_is_valid(evidence, &plan.operation_kind)
    }

    pub(crate) fn is_settled_resource(
        plan: &CatalogOperationPlan,
        evidence: &serde_json::Value,
    ) -> bool {
        if plan.operation_kind != "script-await-resource-font" || !plan.operation_stream.is_empty()
        {
            return false;
        }
        settled_resource_evidence_is_valid(evidence, &plan.operation_kind)
    }

    pub(crate) fn perform(
        session: &mut HtmlBrowserSession,
        plan: &CatalogOperationPlan,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        dispatch::perform(session, plan)
    }
}

#[cfg(test)]
mod tests {
    use super::CatalogOperations;
    use crate::options::CatalogOperationPlan;

    fn settled_plan() -> Result<CatalogOperationPlan, Box<dyn std::error::Error>> {
        Ok(serde_json::from_value(serde_json::json!({
            "profile": "catalog-plan84",
            "conditionId": "async/expected-state/1280x900@1",
            "caseId": "async",
            "state": "expected-state",
            "operationKind": "script-await-timer-microtask",
            "targetSelector": "#state",
            "fixtureSha256": "a".repeat(64),
            "viewport": {"width": 1280, "height": 900, "scale": 1.0},
            "operationEvidencePath": "operation.json",
            "operationEvidenceSha256": "b".repeat(64),
            "evidenceBinarySha256": "c".repeat(64),
            "operationStream": [],
        }))?)
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
        let plan = settled_plan()?;
        assert!(CatalogOperations::is_settled_timer(
            &plan,
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
}
