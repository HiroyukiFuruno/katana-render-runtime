#![cfg(test)]

use crate::{options::OperationPlan, provenance::hex_digest, HtmlBrowserViewport};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) const STATIC_OPERATION_KIND: &str = "script-await-mutation-delivery";
const CATALOG_EVIDENCE_VIEWPORT_WIDTH: u32 = 1280;
const CATALOG_EVIDENCE_VIEWPORT_HEIGHT: u32 = 900;

pub(super) struct Case {
    _temp: tempfile::TempDir,
    pub(super) report_path: PathBuf,
    pub(super) plan: OperationPlan,
    pub(super) source_hash: String,
    pub(super) viewport: HtmlBrowserViewport,
}

pub(super) fn fixture_source_hash() -> Result<String, Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/html/aspects/script-observers.html")
        .canonicalize()?;
    Ok(hex_digest(&fs::read(path)?))
}

pub(super) fn operation_report(source_hash: &str, kind: &str) -> Value {
    operation_report_with_evidence(
        source_hash,
        kind,
        json!({
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
        }),
    )
}

fn operation_report_with_evidence(
    source_hash: &str,
    kind: &str,
    operation_evidence: Value,
) -> Value {
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
        "operationEvidence": operation_evidence
    })
}

pub(super) fn settled_report(source_hash: &str, kind: &str, step: Value) -> Value {
    let operation_evidence = json!({
        "schema": "krr-html84-operation-evidence-v1",
        "kind": kind,
        "status": "observed",
        "sourceSha256Before": source_hash,
        "sourceSha256After": source_hash,
        "preFrame": {"generation": 1, "scrollY": 0},
        "postFrame": {"generation": 1, "scrollY": 0},
        "target": {"selector": "#state", "nodeId": 2, "visible": true},
        "state": {"before": "ready", "after": "ready"},
        "steps": [step],
        "delivery": "session-load-settled"
    });
    operation_report_with_evidence(source_hash, kind, operation_evidence)
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Mutation {
    Status,
    Viewport,
    HtmlSha,
    EvidenceSourceSha,
    TargetVisibility,
    TypedKind,
}

pub(super) fn mutate_report(report: &mut Value, mutation: Mutation) {
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

fn catalog_plan(
    report_path: PathBuf,
    source_hash: &str,
    evidence_hash: String,
    operation_kind: &str,
) -> Result<OperationPlan, Box<dyn std::error::Error>> {
    Ok(serde_json::from_value(json!({
        "profile": "catalog-plan84",
        "conditionId": "observers/expected-state/1280x900@1",
        "caseId": "observers",
        "state": "expected-state",
        "operationKind": operation_kind,
        "targetSelector": "#state",
        "fixtureSha256": source_hash,
        "viewport": {"width": 1280, "height": 900, "scale": 1.0},
        "operationEvidencePath": report_path,
        "operationEvidenceSha256": evidence_hash,
        "evidenceBinarySha256": "b".repeat(64),
        "operationStream": []
    }))?)
}

pub(super) fn case_with_report(
    report: Value,
    source_hash: &str,
    operation_kind: &str,
) -> Result<Case, Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let report_path = temp.path().canonicalize()?.join("catalog-report.json");
    let report_bytes = serde_json::to_vec(&report)?;
    fs::write(&report_path, &report_bytes)?;
    let plan = catalog_plan(
        report_path.clone(),
        source_hash,
        hex_digest(&report_bytes),
        operation_kind,
    )?;
    Ok(Case {
        _temp: temp,
        report_path,
        plan,
        source_hash: source_hash.to_owned(),
        viewport: HtmlBrowserViewport::new(
            CATALOG_EVIDENCE_VIEWPORT_WIDTH,
            CATALOG_EVIDENCE_VIEWPORT_HEIGHT,
            1.0,
        )?,
    })
}

pub(super) fn case_with_static_report() -> Result<Case, Box<dyn std::error::Error>> {
    let source_hash = fixture_source_hash()?;
    case_with_static_report_for_hash(&source_hash)
}

pub(super) fn case_with_static_report_for_hash(
    source_hash: &str,
) -> Result<Case, Box<dyn std::error::Error>> {
    case_with_report(
        operation_report(source_hash, STATIC_OPERATION_KIND),
        source_hash,
        STATIC_OPERATION_KIND,
    )
}

pub(super) fn case_with_mutated_static_report(
    source_hash: &str,
    mutation: Mutation,
) -> Result<Case, Box<dyn std::error::Error>> {
    let mut report = operation_report(source_hash, STATIC_OPERATION_KIND);
    mutate_report(&mut report, mutation);
    case_with_report(report, source_hash, STATIC_OPERATION_KIND)
}
