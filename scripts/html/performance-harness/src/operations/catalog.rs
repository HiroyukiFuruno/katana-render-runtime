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
#[path = "catalog_tests.rs"]
mod tests;
