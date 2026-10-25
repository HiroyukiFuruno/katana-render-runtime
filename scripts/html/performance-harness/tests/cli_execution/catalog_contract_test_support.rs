#![cfg(test)]

use super::{Inputs, TempDir};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

pub(super) const VIEWPORT_SCALE: f32 = 1.0;
pub(super) const WORKER_SAMPLE: usize = 0;
const TEST_NODE_ID: u64 = 1;
pub(super) const STATIC_OPERATION_KIND: &str = "script-await-mutation-delivery";
pub(super) const EXPECTED_TARGET_SELECTOR: &str = "#state";
const SYNTHETIC_HTML: &str =
    "<!doctype html><html><body><button id=\"state\">unit-only fixture</button></body></html>";

pub(super) struct CatalogContractInputs {
    pub(super) html_path: PathBuf,
    pub(super) plan_path: PathBuf,
    pub(super) output_path: PathBuf,
    pub(super) source_sha256: String,
}

pub(super) fn prepare_catalog_cli_inputs(
    temp: &TempDir,
    requested_selector: &str,
) -> Result<(Inputs, CatalogContractInputs), Box<dyn std::error::Error>> {
    let mut cli_inputs = super::valid_inputs(temp)?;
    let catalog_inputs = write_inputs(temp.path(), requested_selector)?;
    cli_inputs.fixture = catalog_inputs.html_path.clone();
    cli_inputs.plan = catalog_inputs.plan_path.clone();
    cli_inputs.fixture_sha = catalog_inputs.source_sha256.clone();
    Ok((cli_inputs, catalog_inputs))
}

pub(super) fn write_inputs(
    directory: &Path,
    requested_selector: &str,
) -> Result<CatalogContractInputs, Box<dyn std::error::Error>> {
    let html = SYNTHETIC_HTML.as_bytes();
    let html_path = directory.join("unit-only-catalog-input.html");
    fs::write(&html_path, html)?;
    let source_sha256 = super::sha(html);
    let binary_sha256 = super::sha(b"unit-only synthetic catalog contract input");
    let report_bytes = serde_json::to_vec(&catalog_report(&source_sha256, &binary_sha256))?;
    let report_path = directory
        .canonicalize()?
        .join("unit-only-catalog-evidence.json");
    fs::write(&report_path, &report_bytes)?;
    let plan = catalog_plan(
        requested_selector,
        &report_path,
        &source_sha256,
        &super::sha(&report_bytes),
        &binary_sha256,
    );
    let plan_path = directory.join("unit-only-catalog-plan.json");
    fs::write(&plan_path, serde_json::to_vec(&plan)?)?;
    Ok(CatalogContractInputs {
        html_path,
        plan_path,
        output_path: directory.join("unused-worker-output.json"),
        source_sha256,
    })
}

pub(super) fn replace_plan_fixture_hash(
    path: &Path,
    fixture_hash: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    update_plan(path, |plan| {
        plan["fixtureSha256"] = Value::String(fixture_hash.to_owned());
    })
}

pub(super) fn replace_plan_viewport_width(
    path: &Path,
    width: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    update_plan(path, |plan| plan["viewport"]["width"] = json!(width))
}

fn update_plan(
    path: &Path,
    update: impl FnOnce(&mut Value),
) -> Result<(), Box<dyn std::error::Error>> {
    let mut plan: Value = serde_json::from_slice(&fs::read(path)?)?;
    update(&mut plan);
    fs::write(path, serde_json::to_vec(&plan)?)?;
    Ok(())
}

// WHY: validatorのwire契約を確かめる一時入力であり、実観測・受入証拠として扱わない。
fn catalog_report(source_sha256: &str, binary_sha256: &str) -> Value {
    json!({
        "schema": "krr-html84-raw-condition-v1",
        "status": "observed",
        "conditionId": "unit-only-catalog/expected-state/960x720@1",
        "caseId": "unit-only-catalog",
        "boundaryState": "expected-state",
        "operationKind": STATIC_OPERATION_KIND,
        "executionNonce": "unit-only-catalog-contract-input",
        "htmlSha256": source_sha256,
        "binarySha256": binary_sha256,
        "viewport": {"width": super::WIDTH, "height": super::HEIGHT, "scale": VIEWPORT_SCALE},
        "operationEvidence": catalog_operation_evidence(source_sha256),
    })
}

fn catalog_operation_evidence(source_sha256: &str) -> Value {
    json!({
        "schema": "krr-html84-operation-evidence-v1",
        "kind": STATIC_OPERATION_KIND,
        "status": "observed",
        "sourceSha256Before": source_sha256,
        "sourceSha256After": source_sha256,
        "target": {
            "selector": EXPECTED_TARGET_SELECTOR,
            "nodeId": TEST_NODE_ID,
            "visible": true
        },
        "state": {"before": "ready", "after": "ready"},
        "steps": [{
            "kind": "session-load-settled-observer-state",
            "stateBefore": "ready",
            "stateAfter": "ready"
        }],
        "delivery": "session-load-settled",
    })
}

fn catalog_plan(
    requested_selector: &str,
    report_path: &Path,
    source_sha256: &str,
    report_sha256: &str,
    binary_sha256: &str,
) -> Value {
    json!({
        "profile": "catalog-plan84",
        "conditionId": "unit-only-catalog/expected-state/960x720@1",
        "caseId": "unit-only-catalog",
        "state": "expected-state",
        "operationKind": STATIC_OPERATION_KIND,
        "targetSelector": requested_selector,
        "fixtureSha256": source_sha256,
        "viewport": {"width": super::WIDTH, "height": super::HEIGHT, "scale": VIEWPORT_SCALE},
        "operationEvidencePath": report_path,
        "operationEvidenceSha256": report_sha256,
        "evidenceBinarySha256": binary_sha256,
        "operationStream": [],
    })
}

pub(super) fn run_worker(
    inputs: &CatalogContractInputs,
) -> Result<Output, Box<dyn std::error::Error>> {
    Ok(Command::new(super::BIN)
        .arg("--worker")
        .arg(WORKER_SAMPLE.to_string())
        .arg(&inputs.html_path)
        .arg(&inputs.output_path)
        .arg(&inputs.plan_path)
        .arg(super::WIDTH.to_string())
        .arg(super::HEIGHT.to_string())
        .arg(VIEWPORT_SCALE.to_string())
        .output()?)
}

pub(super) fn assert_success(output: &Output) -> Result<(), Box<dyn std::error::Error>> {
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    Ok(())
}

pub(super) fn option_args(numbers: &[&str]) -> Vec<String> {
    let mut args = vec![
        "unit-only-input.html".to_owned(),
        "unit-only-output.json".to_owned(),
        "unit-only-plan.json".to_owned(),
    ];
    args.extend(numbers.iter().map(|value| (*value).to_owned()));
    args
}

pub(super) fn option_error_cases() -> Vec<(Vec<String>, &'static str)> {
    let width = super::WIDTH.to_string();
    let height = super::HEIGHT.to_string();
    vec![
        (Vec::new(), "viewport width is required"),
        (vec![width.clone()], "viewport height is required"),
        (vec!["not-u32".to_owned()], "ParseIntError"),
        (
            vec![width.clone(), height.clone()],
            "viewport scale is required",
        ),
        (vec![width, height, "not-f32".to_owned()], "ParseFloatError"),
        (
            vec![
                super::WIDTH.to_string(),
                super::HEIGHT.to_string(),
                VIEWPORT_SCALE.to_string(),
                "unit-only-trailing-argument".to_owned(),
            ],
            "unexpected trailing argument",
        ),
    ]
}

pub(super) fn run_cli_args(args: &[String]) -> Result<Output, Box<dyn std::error::Error>> {
    Ok(Command::new(super::BIN).args(args).output()?)
}
