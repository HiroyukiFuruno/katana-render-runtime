#![cfg(test)]

use super::{BIN, HEIGHT, WIDTH};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

pub(super) type TempDir = tempfile::TempDir;

pub(super) struct Inputs {
    pub(super) fixture: PathBuf,
    pub(super) plan: PathBuf,
    pub(super) evidence: PathBuf,
    pub(super) snapshot: PathBuf,
    pub(super) fixture_sha: String,
    pub(super) evidence_bytes: Vec<u8>,
}

pub(super) fn create_temp_dir() -> Result<TempDir, Box<dyn std::error::Error>> {
    Ok(tempfile::Builder::new().prefix("krr-perf-cli-").tempdir()?)
}

pub(super) fn valid_inputs(temp: &TempDir) -> Result<Inputs, Box<dyn std::error::Error>> {
    let fixture = fixture_path()?;
    let fixture_sha = super::sha(&fs::read(&fixture)?);
    let evidence = temp.path().join("unit-only-dom-state.json");
    let evidence_bytes =
        br#"{"scope":"unit-test-only","states":{"js":{"expanded":true,"result":"opened"}}}"#
            .to_vec();
    fs::write(&evidence, &evidence_bytes)?;
    let snapshot = temp.path().join("unit-only-source-snapshot.json");
    fs::write(
        &snapshot,
        br#"{"scope":"unit-test-only; not release evidence"}"#,
    )?;
    let plan = temp.path().join("operation-plan.json");
    write_plan(&plan, &fixture_sha, &evidence, &evidence_bytes)?;
    Ok(Inputs {
        fixture,
        plan,
        evidence,
        snapshot,
        fixture_sha,
        evidence_bytes,
    })
}

pub(super) fn fixture_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/html/interaction-fidelity.html")
        .canonicalize()?)
}

fn write_plan(
    path: &Path,
    fixture_sha: &str,
    evidence: &Path,
    evidence_bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let plan = serde_json::json!({
        "fixture_sha256": fixture_sha, "operation": "action-button-toggle",
        "x": 100.0, "y": 100.0, "expected_dom_state_path": evidence,
        "expected_dom_state_sha256": super::sha(evidence_bytes),
    });
    fs::write(path, serde_json::to_vec(&plan)?)?;
    Ok(())
}

pub(super) fn cli(inputs: &Inputs, output: &Path) -> Result<Output, Box<dyn std::error::Error>> {
    let lock = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Cargo.lock")
        .canonicalize()?;
    cli_with_provenance_paths(inputs, output, &lock, &inputs.snapshot)
}

pub(super) fn cli_with_provenance_paths(
    inputs: &Inputs,
    output: &Path,
    lock_path: &Path,
    source_snapshot_path: &Path,
) -> Result<Output, Box<dyn std::error::Error>> {
    Ok(provenance_command(inputs, output)
        .env("KRR_PERF_LOCK_PATH", lock_path)
        .env("KRR_PERF_SOURCE_SNAPSHOT_PATH", source_snapshot_path)
        .output()?)
}

pub(super) fn cli_with_missing_required_provenance_env(
    inputs: &Inputs,
    output: &Path,
    missing_env: &str,
) -> Result<Output, Box<dyn std::error::Error>> {
    let lock_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Cargo.lock")
        .canonicalize()?;
    Ok(provenance_command(inputs, output)
        .env("KRR_PERF_LOCK_PATH", lock_path)
        .env("KRR_PERF_SOURCE_SNAPSHOT_PATH", &inputs.snapshot)
        .env_remove(missing_env)
        .output()?)
}

fn provenance_command(inputs: &Inputs, output: &Path) -> Command {
    let mut command = Command::new(BIN);
    command
        .arg(&inputs.fixture)
        .arg(output)
        .arg(&inputs.plan)
        .arg(WIDTH.to_string())
        .arg(HEIGHT.to_string())
        .arg("1")
        .env("KRR_PERF_PAIR_INDEX", "37")
        .env("KRR_PERF_MEASUREMENT_EXECUTED", "0")
        .env("KRR_PERF_RUSTC", "unit-test-only")
        .env("KRR_PERF_CARGO", "unit-test-only")
        .env("KRR_PERF_ENVIRONMENT_SHA256", "unit-test-only");
    command
}
