use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const BIN: &str = env!("CARGO_BIN_EXE_krr-html-performance-harness");
const WIDTH: u32 = 960;
const HEIGHT: u32 = 720;
#[cfg(feature = "candidate")]
#[path = "cli_execution/native_frame_capture.rs"]
mod native_frame_capture;
#[cfg(feature = "candidate")]
#[path = "cli_execution/retention_contract_tests.rs"]
mod retention_contract_tests;
#[cfg(feature = "baseline")]
const ARM: &str = "baseline-0.4.23";
#[cfg(feature = "candidate")]
const ARM: &str = "candidate";

#[test]
fn cli_measures_five_real_workers_and_records_unit_only_provenance(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = create_temp_dir()?;
    let inputs = valid_inputs(&temp)?;
    let output = temp.path().join("measurement.json");
    let result = cli(&inputs, &output)?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&fs::read(output)?)?;
    assert_report(&report, &inputs)?;
    Ok(())
}

#[test]
fn cli_rejects_malformed_plan_without_report() -> Result<(), Box<dyn std::error::Error>> {
    let temp = create_temp_dir()?;
    let fixture = fixture_path()?;
    let plan = temp.path().join("invalid-plan.json");
    let snapshot = temp.path().join("unit-only-snapshot.json");
    fs::write(&plan, b"{not-json")?;
    fs::write(&snapshot, b"unit-test-only")?;
    let inputs = Inputs {
        fixture,
        plan,
        evidence: temp.path().join("unused-evidence.json"),
        snapshot,
        fixture_sha: String::new(),
        evidence_bytes: Vec::new(),
    };
    let output = temp.path().join("must-not-exist.json");
    let result = cli(&inputs, &output)?;
    assert!(!result.status.success());
    assert!(!output.exists());
    assert!(!result.stderr.is_empty());
    Ok(())
}

fn assert_report(report: &Value, inputs: &Inputs) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(report["arm"], ARM);
    assert_eq!(report["measurement_executed"], false);
    assert_eq!(report["pair_index"], 37);
    assert_eq!(report["source_sha256"], inputs.fixture_sha);
    assert_eq!(report["operation"], "action-button-toggle");
    assert_eq!(report["sample_count"], 5);
    let samples = report["samples"]
        .as_array()
        .ok_or("missing worker samples")?;
    assert_eq!(samples.len(), 5);
    for (index, sample) in samples.iter().enumerate() {
        assert_sample(sample, index, &inputs.fixture_sha)?;
    }
    assert_provenance(report, inputs)?;
    Ok(())
}

fn assert_sample(
    sample: &Value,
    index: usize,
    fixture_sha: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let phases = &sample["phases"];
    let origin = format!("file:///krr-performance/{fixture_sha}.html");
    assert_eq!(sample["sample"], index);
    for phase in ["cold_open", "warm_open", "re_display", "full_y"] {
        assert_frame(&phases[phase]["frame"], &origin)?;
    }
    let bottom = &phases["full_y"];
    assert_eq!(bottom["actual_bottom"], true);
    assert_eq!(bottom["final_scroll_y"], bottom["max_scroll"]);
    assert_eq!(bottom["frame"]["scroll_y"], bottom["max_scroll"]);
    assert!(bottom["max_scroll"].as_f64().ok_or("missing max scroll")? > 0.0);
    assert_interaction(&phases["post_operation"], &origin)?;
    assert_resize(&phases["same_session_resize"], &origin)?;
    assert_generations(phases)?;
    Ok(())
}

fn assert_frame(frame: &Value, origin: &str) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(frame["origin"], origin);
    assert!(
        frame["generation"]
            .as_u64()
            .ok_or("missing frame generation")?
            > 0
    );
    assert!(
        frame["pixel_bytes"]
            .as_u64()
            .ok_or("missing frame pixels")?
            > 0
    );
    assert_eq!(frame["viewport"]["width"], WIDTH);
    assert_eq!(frame["viewport"]["height"], HEIGHT);
    Ok(())
}

fn assert_interaction(operation: &Value, origin: &str) -> Result<(), Box<dyn std::error::Error>> {
    let frame = &operation["measurement"]["frame"];
    assert_frame(frame, origin)?;
    assert_eq!(frame["scroll_y"], 0.0);
    assert_eq!(operation["visual_change_observed"], true);
    assert!(
        operation["generation_after"]
            .as_u64()
            .ok_or("missing operation generation")?
            > operation["generation_before"]
                .as_u64()
                .ok_or("missing prior generation")?
    );
    assert_ne!(operation["pixel_sha_before"], operation["pixel_sha_after"]);
    assert_eq!(
        operation["semantic_state_evidence"]["lane"],
        "existing-html-interaction-regression"
    );
    Ok(())
}

fn assert_resize(resize: &Value, origin: &str) -> Result<(), Box<dyn std::error::Error>> {
    let evidence = &resize["resize_evidence"];
    let frame = &resize["measurement"]["frame"];
    assert_eq!(evidence["real_raster_frame"], true);
    assert_eq!(evidence["from_viewport"]["width"], WIDTH);
    assert!(
        evidence["to_viewport"]["width"]
            .as_u64()
            .ok_or("missing resized width")?
            < u64::from(WIDTH)
    );
    assert_eq!(frame["origin"], origin);
    assert_eq!(frame["viewport"]["width"], evidence["to_viewport"]["width"]);
    assert!(
        frame["pixel_bytes"]
            .as_u64()
            .ok_or("missing resized pixels")?
            > 0
    );
    Ok(())
}

fn assert_generations(phases: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let generations = [
        phases["re_display"]["frame"]["generation"]
            .as_u64()
            .ok_or("missing re-display generation")?,
        phases["full_y"]["frame"]["generation"]
            .as_u64()
            .ok_or("missing full-Y generation")?,
        phases["post_operation"]["generation_after"]
            .as_u64()
            .ok_or("missing interaction generation")?,
        phases["same_session_resize"]["resize_evidence"]["generation_after"]
            .as_u64()
            .ok_or("missing resize generation")?,
    ];
    assert!(generations.windows(2).all(|pair| pair[0] < pair[1]));
    Ok(())
}

fn assert_provenance(report: &Value, inputs: &Inputs) -> Result<(), Box<dyn std::error::Error>> {
    let provenance = &report["provenance"];
    let lock_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Cargo.lock")
        .canonicalize()?;
    assert_artifact_pins(provenance, &lock_path)?;
    assert_source_pins(report, provenance, inputs)?;
    assert_provenance_paths(provenance, inputs, &lock_path);
    assert_unit_only_provenance(provenance);
    assert!(inputs.evidence.exists());
    Ok(())
}

fn assert_artifact_pins(
    provenance: &Value,
    lock_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(provenance["binary_sha256"], sha(&fs::read(BIN)?));
    assert_eq!(provenance["deps_lock_sha256"], sha(&fs::read(lock_path)?));
    Ok(())
}

fn assert_source_pins(
    report: &Value,
    provenance: &Value,
    inputs: &Inputs,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        provenance["source_snapshot_sha256"],
        sha(&fs::read(&inputs.snapshot)?)
    );
    assert_eq!(
        report["expected_dom_state_sha256"],
        sha(&inputs.evidence_bytes)
    );
    Ok(())
}

fn assert_provenance_paths(provenance: &Value, inputs: &Inputs, lock_path: &Path) {
    assert_eq!(
        provenance["deps_lock_path"],
        lock_path.to_string_lossy().as_ref()
    );
    assert_eq!(
        provenance["source_snapshot_path"],
        inputs.snapshot.to_string_lossy().as_ref()
    );
}

fn assert_unit_only_provenance(provenance: &Value) {
    assert_eq!(provenance["rustc"], "unit-test-only");
    assert_eq!(provenance["cargo"], "unit-test-only");
    assert_eq!(provenance["environment_sha256"], "unit-test-only");
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[path = "cli_execution/legacy_guard_regressions.rs"]
mod legacy_guard_regressions;

#[path = "cli_execution/provenance_regressions.rs"]
mod provenance_regressions;

#[path = "cli_execution/provenance_test_support.rs"]
mod provenance_test_support;

#[path = "cli_execution/catalog_contract_test_support.rs"]
mod catalog_contract_test_support;

#[path = "cli_execution/catalog_contract_tests.rs"]
mod catalog_contract_tests;

use provenance_test_support::{
    cli, cli_with_missing_required_provenance_env, cli_with_provenance_paths, create_temp_dir,
    fixture_path, valid_inputs, Inputs, TempDir,
};
