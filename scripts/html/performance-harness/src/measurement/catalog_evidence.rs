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
mod tests {
    use super::validate_sample_input;
    use crate::{options::OperationPlan, provenance::hex_digest, HtmlBrowserViewport};
    use serde_json::{json, Value};
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    const STATIC_OPERATION_KIND: &str = "script-await-mutation-delivery";

    struct Case {
        _temp: tempfile::TempDir,
        report_path: PathBuf,
        plan: OperationPlan,
        source_hash: String,
        viewport: HtmlBrowserViewport,
    }

    fn fixture_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/html/aspects/script-observers.html")
            .canonicalize()?)
    }

    fn operation_report(source_hash: &str, kind: &str) -> Value {
        json!({
            "schema": "krr-html84-raw-condition-v1",
            "status": "observed",
            "conditionId": "observers/expected-state/1280x900@1",
            "caseId": "observers",
            "boundaryState": "expected-state",
            "operationKind": kind,
            "executionNonce": "unit-only",
            "htmlSha256": source_hash,
            "binarySha256": "b".repeat(64),
            "viewport": {"width": 1280, "height": 900, "scale": 1.0},
            "operationEvidence": {
                "schema": "krr-html84-operation-evidence-v1",
                "kind": kind,
                "status": "observed",
                "sourceSha256Before": source_hash,
                "sourceSha256After": source_hash,
                "target": {"selector": "#state", "nodeId": 2, "visible": true},
                "state": {"before": "ready", "after": "ready"},
                "steps": [{
                    "kind": "session-load-settled-observer-state",
                    "stateBefore": "ready",
                    "stateAfter": "ready"
                }],
                "delivery": "session-load-settled"
            }
        })
    }

    #[derive(Clone, Copy, Debug)]
    enum Mutation {
        Status,
        Viewport,
        HtmlSha,
        EvidenceSourceSha,
        TargetVisibility,
        TypedKind,
    }

    fn mutate_report(report: &mut Value, mutation: Mutation) {
        match mutation {
            Mutation::Status => report["status"] = json!("unknown"),
            Mutation::Viewport => report["viewport"]["scale"] = json!(2.0),
            Mutation::HtmlSha => report["htmlSha256"] = json!("0".repeat(64)),
            Mutation::EvidenceSourceSha => {
                report["operationEvidence"]["sourceSha256After"] = json!("0".repeat(64));
            }
            Mutation::TargetVisibility => {
                report["operationEvidence"]["target"]["visible"] = json!(false)
            }
            Mutation::TypedKind => report["operationEvidence"]["kind"] = json!("other-kind"),
        }
    }

    fn fixture_source_hash() -> Result<String, Box<dyn std::error::Error>> {
        Ok(hex_digest(&fs::read(fixture_path()?)?))
    }

    fn catalog_plan(
        report_path: PathBuf,
        source_hash: &str,
        evidence_hash: String,
    ) -> Result<OperationPlan, Box<dyn std::error::Error>> {
        Ok(serde_json::from_value(json!({
            "profile": "catalog-plan84",
            "conditionId": "observers/expected-state/1280x900@1",
            "caseId": "observers",
            "state": "expected-state",
            "operationKind": STATIC_OPERATION_KIND,
            "targetSelector": "#state",
            "fixtureSha256": source_hash,
            "viewport": {"width": 1280, "height": 900, "scale": 1.0},
            "operationEvidencePath": report_path,
            "operationEvidenceSha256": evidence_hash,
            "evidenceBinarySha256": "b".repeat(64),
            "operationStream": []
        }))?)
    }

    fn case_with_report(
        report: Value,
        source_hash: &str,
    ) -> Result<Case, Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let report_path = temp.path().canonicalize()?.join("catalog-report.json");
        let report_bytes = serde_json::to_vec(&report)?;
        fs::write(&report_path, &report_bytes)?;
        let evidence_hash = hex_digest(&report_bytes);
        let plan = catalog_plan(report_path.clone(), source_hash, evidence_hash)?;
        let viewport = HtmlBrowserViewport::new(1280, 900, 1.0)?;
        Ok(Case {
            _temp: temp,
            report_path,
            plan,
            source_hash: source_hash.to_owned(),
            viewport,
        })
    }

    fn case_with_static_report() -> Result<Case, Box<dyn std::error::Error>> {
        let source_hash = fixture_source_hash()?;
        case_with_static_report_for_hash(&source_hash)
    }

    fn case_with_static_report_for_hash(
        source_hash: &str,
    ) -> Result<Case, Box<dyn std::error::Error>> {
        case_with_report(
            operation_report(source_hash, STATIC_OPERATION_KIND),
            source_hash,
        )
    }

    fn case_with_mutated_static_report(
        source_hash: &str,
        mutation: Mutation,
    ) -> Result<Case, Box<dyn std::error::Error>> {
        let mut report = operation_report(source_hash, STATIC_OPERATION_KIND);
        mutate_report(&mut report, mutation);
        case_with_report(report, source_hash)
    }

    #[test]
    fn validates_observed_catalog_report_from_real_fixture_and_file(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let case = case_with_static_report()?;
        assert!(case.report_path.is_file());
        assert!(!validate_sample_input(
            &case.plan,
            &case.source_hash,
            case.viewport
        )?);
        Ok(())
    }

    #[test]
    fn rejects_catalog_identity_viewport_typed_fact_and_target_mutations(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source_hash = fixture_source_hash()?;
        for mutation in [
            Mutation::Status,
            Mutation::Viewport,
            Mutation::HtmlSha,
            Mutation::EvidenceSourceSha,
            Mutation::TargetVisibility,
            Mutation::TypedKind,
        ] {
            let case = case_with_mutated_static_report(&source_hash, mutation)?;
            assert!(
                validate_sample_input(&case.plan, &case.source_hash, case.viewport).is_err(),
                "tampered catalog evidence was accepted: {mutation:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn rejects_evidence_changed_after_plan_hash_was_pinned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let case = case_with_static_report()?;
        fs::write(&case.report_path, b"tampered after the plan was written")?;
        assert!(validate_sample_input(&case.plan, &case.source_hash, case.viewport).is_err());
        Ok(())
    }

    #[test]
    fn rejects_plan_when_fixture_sha_does_not_match_input() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut case = case_with_static_report()?;
        if let OperationPlan::Catalog(plan) = &mut case.plan {
            plan.fixture_sha256 = "0".repeat(64);
        }
        assert!(validate_sample_input(&case.plan, &case.source_hash, case.viewport).is_err());
        Ok(())
    }

    #[test]
    fn rejects_invalid_json_in_a_correctly_pinned_report_file(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut case = case_with_static_report()?;
        let bytes = b"{invalid json";
        fs::write(&case.report_path, bytes)?;
        if let OperationPlan::Catalog(plan) = &mut case.plan {
            plan.operation_evidence_sha256 = hex_digest(bytes);
        }
        assert!(validate_sample_input(&case.plan, &case.source_hash, case.viewport).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_catalog_evidence_path() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let mut case = case_with_static_report()?;
        let alias = case.report_path.with_file_name("catalog-report-alias.json");
        symlink(&case.report_path, &alias)?;
        if let OperationPlan::Catalog(plan) = &mut case.plan {
            plan.operation_evidence_path = alias;
        }
        assert!(validate_sample_input(&case.plan, &case.source_hash, case.viewport).is_err());
        Ok(())
    }
}
