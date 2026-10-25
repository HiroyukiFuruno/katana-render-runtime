use std::fs;

use super::super::operations::CatalogOperations;
use super::super::options::{CatalogOperationPlan, OperationPlan};
use super::super::{hex_digest, HtmlBrowserViewport};

pub(super) fn catalog_condition_report(operation_plan: &OperationPlan) -> serde_json::Value {
    match operation_plan {
        OperationPlan::Legacy(_) => serde_json::Value::Null,
        OperationPlan::Catalog(plan) => serde_json::json!({
            "profile": plan.profile,
            "condition_id": plan.condition_id,
            "case_id": plan.case_id,
            "state": plan.state,
            "operation_kind": plan.operation_kind,
            "operation_evidence_sha256": plan.operation_evidence_sha256,
        }),
    }
}

pub(super) fn validate_sample_input(
    operation_plan: &OperationPlan,
    source_hash: &str,
    viewport: HtmlBrowserViewport,
) -> Result<bool, Box<dyn std::error::Error>> {
    if operation_plan.fixture_sha256() != source_hash {
        return Err("operation plan fixture SHA does not match input".into());
    }
    if let OperationPlan::Catalog(plan) = operation_plan {
        return validate_catalog_evidence(plan, source_hash, viewport);
    }
    Ok(false)
}

fn validate_catalog_evidence(
    plan: &CatalogOperationPlan,
    source_hash: &str,
    viewport: HtmlBrowserViewport,
) -> Result<bool, Box<dyn std::error::Error>> {
    let report = read_catalog_report(plan)?;
    validate_catalog_report_identity(&report, plan, source_hash, viewport)?;
    let evidence = report
        .get("operationEvidence")
        .ok_or("catalog typed operation evidence is missing")?;
    validate_typed_operation_evidence(evidence, plan, source_hash)?;
    validate_catalog_target(evidence, plan)?;
    validate_catalog_operation_stream(plan, evidence)
}

fn read_catalog_report(
    plan: &CatalogOperationPlan,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let path = &plan.operation_evidence_path;
    if path.is_symlink() || path.canonicalize()? != *path {
        return Err("catalog operation evidence path must be a physical file".into());
    }
    let raw = fs::read(path)?;
    if hex_digest(&raw) != plan.operation_evidence_sha256 {
        return Err("catalog operation evidence SHA changed before measurement".into());
    }
    Ok(serde_json::from_slice(&raw)?)
}

fn validate_catalog_report_identity(
    report: &serde_json::Value,
    plan: &CatalogOperationPlan,
    source_hash: &str,
    viewport: HtmlBrowserViewport,
) -> Result<(), Box<dyn std::error::Error>> {
    let observed_viewport = report
        .get("viewport")
        .ok_or("catalog operation report viewport is missing")?;
    if !report_has_observed_identity(report, plan, source_hash)
        || !report_has_requested_viewport(observed_viewport, viewport)
    {
        return Err("catalog operation evidence status or identity differs".into());
    }
    Ok(())
}

fn report_has_observed_identity(
    report: &serde_json::Value,
    plan: &CatalogOperationPlan,
    source_hash: &str,
) -> bool {
    report.get("status").and_then(serde_json::Value::as_str) == Some("observed")
        && report.get("unknownReason").is_none()
        && report
            .get("conditionId")
            .and_then(serde_json::Value::as_str)
            == Some(plan.condition_id.as_str())
        && report.get("caseId").and_then(serde_json::Value::as_str) == Some(plan.case_id.as_str())
        && report
            .get("boundaryState")
            .and_then(serde_json::Value::as_str)
            == Some(plan.state.as_str())
        && report
            .get("operationKind")
            .and_then(serde_json::Value::as_str)
            == Some(plan.operation_kind.as_str())
        && report.get("htmlSha256").and_then(serde_json::Value::as_str) == Some(source_hash)
        && report
            .get("binarySha256")
            .and_then(serde_json::Value::as_str)
            == Some(plan.evidence_binary_sha256.as_str())
}

fn report_has_requested_viewport(
    observed_viewport: &serde_json::Value,
    viewport: HtmlBrowserViewport,
) -> bool {
    let Some(observed_viewport) = observed_viewport.as_object() else {
        return false;
    };
    let width = observed_viewport
        .get("width")
        .and_then(serde_json::Value::as_u64);
    let height = observed_viewport
        .get("height")
        .and_then(serde_json::Value::as_u64);
    let scale = observed_viewport
        .get("scale")
        .and_then(serde_json::Value::as_f64);
    width == Some(u64::from(viewport.width))
        && height == Some(u64::from(viewport.height))
        && scale == Some(f64::from(viewport.device_scale_factor))
}

fn validate_typed_operation_evidence(
    evidence: &serde_json::Value,
    plan: &CatalogOperationPlan,
    source_hash: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if evidence.get("schema").and_then(serde_json::Value::as_str)
        != Some("krr-html84-operation-evidence-v1")
        || evidence.get("kind").and_then(serde_json::Value::as_str)
            != Some(plan.operation_kind.as_str())
        || evidence
            .get("sourceSha256Before")
            .and_then(serde_json::Value::as_str)
            != Some(source_hash)
        || evidence
            .get("sourceSha256After")
            .and_then(serde_json::Value::as_str)
            != Some(source_hash)
    {
        return Err("catalog operation evidence contract differs".into());
    }
    Ok(())
}

fn validate_catalog_target(
    evidence: &serde_json::Value,
    plan: &CatalogOperationPlan,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = evidence
        .get("target")
        .ok_or("catalog operation target evidence is missing")?;
    if target.get("visible").and_then(serde_json::Value::as_bool) != Some(true)
        || target
            .get("nodeId")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        || plan.target_selector.as_deref().is_some_and(|selector| {
            target.get("selector").and_then(serde_json::Value::as_str) != Some(selector)
        })
    {
        return Err("catalog operation target evidence differs".into());
    }
    Ok(())
}

fn validate_catalog_operation_stream(
    plan: &CatalogOperationPlan,
    evidence: &serde_json::Value,
) -> Result<bool, Box<dyn std::error::Error>> {
    if CatalogOperations::is_static(plan) {
        if !plan.operation_stream.is_empty() {
            return Err("catalog operation stream does not match its finite operation kind".into());
        }
        return Ok(false);
    }
    if CatalogOperations::is_settled_timer(plan, evidence) {
        return Ok(true);
    }
    if CatalogOperations::is_settled_resource(plan, evidence) {
        return Ok(true);
    }
    if plan.operation_stream.is_empty() {
        return Err("catalog operation stream does not match its finite operation kind".into());
    }
    Ok(false)
}

#[cfg(test)]
#[path = "catalog_evidence_tests.rs"]
mod tests;
