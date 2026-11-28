#![cfg(test)]

use super::{
    catalog_contract_test_support::{
        assert_success, option_args, option_error_cases, prepare_catalog_cli_inputs,
        replace_plan_fixture_hash, replace_plan_viewport_width, run_cli_args, run_worker,
        write_inputs, STATIC_OPERATION_KIND,
    },
    create_temp_dir,
};
use serde_json::Value;

#[test]
fn cli_worker_accepts_a_matching_synthetic_catalog_target_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = create_temp_dir()?;
    let inputs = write_inputs(
        temp.path(),
        super::catalog_contract_test_support::EXPECTED_TARGET_SELECTOR,
    )?;
    let result = run_worker(&inputs)?;
    assert_success(&result)?;
    let sample: Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(
        sample["sample"],
        super::catalog_contract_test_support::WORKER_SAMPLE
    );
    assert_eq!(
        sample["catalog_condition"]["operation_kind"],
        STATIC_OPERATION_KIND
    );
    let expected_origin = format!("file:///krr-performance/{}.html", inputs.source_sha256);
    assert_eq!(
        sample["phases"]["cold_open"]["frame"]["origin"],
        expected_origin
    );
    Ok(())
}

#[test]
fn cli_catalog_report_uses_metadata_from_a_full_measurement_run(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = create_temp_dir()?;
    let (inputs, catalog_inputs) = prepare_catalog_cli_inputs(
        &temp,
        super::catalog_contract_test_support::EXPECTED_TARGET_SELECTOR,
    )?;
    let output_path = temp.path().join("unit-only-catalog-measurement.json");
    let result = super::cli(&inputs, &output_path)?;
    assert_success(&result)?;

    let report: Value = serde_json::from_slice(&std::fs::read(output_path)?)?;
    assert_catalog_report_metadata(&report, &catalog_inputs.source_sha256)
}

#[test]
fn cli_rejects_a_catalog_plan_with_a_mismatched_viewport() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = create_temp_dir()?;
    let (inputs, catalog_inputs) = prepare_catalog_cli_inputs(
        &temp,
        super::catalog_contract_test_support::EXPECTED_TARGET_SELECTOR,
    )?;
    replace_plan_viewport_width(&catalog_inputs.plan_path, super::WIDTH + super::HEIGHT)?;
    let output_path = temp
        .path()
        .join("unit-only-invalid-catalog-measurement.json");
    let result = super::cli(&inputs, &output_path)?;
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{stderr}");
    assert!(stderr.contains("catalog operation plan does not match the worker viewport"));
    assert!(result.stdout.is_empty());
    assert!(!output_path.exists());
    Ok(())
}

fn assert_catalog_report_metadata(
    report: &Value,
    source_sha256: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(report["measurement_executed"], false);
    assert_eq!(report["source_sha256"], source_sha256);
    assert_eq!(report["operation"], STATIC_OPERATION_KIND);
    assert_eq!(report["expected_dom_state_sha256"], "");
    assert_eq!(
        report["catalog_condition"]["operation_kind"],
        STATIC_OPERATION_KIND
    );
    assert_eq!(report["catalog_condition"]["target_selector"], "#state");
    assert_eq!(report["sample_count"], 5);
    assert_eq!(
        report["samples"]
            .as_array()
            .ok_or("missing worker samples")?
            .len(),
        5
    );
    assert_eq!(report["provenance"]["rustc"], "unit-test-only");
    Ok(())
}

#[test]
fn cli_worker_rejects_a_mismatched_synthetic_catalog_target_selector(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = create_temp_dir()?;
    let inputs = write_inputs(temp.path(), "#different")?;
    let result = run_worker(&inputs)?;
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr)
        .contains("catalog operation target evidence differs"));
    Ok(())
}

#[test]
fn cli_worker_rejects_a_synthetic_plan_with_a_mismatched_fixture_hash(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = create_temp_dir()?;
    let inputs = write_inputs(
        temp.path(),
        super::catalog_contract_test_support::EXPECTED_TARGET_SELECTOR,
    )?;
    let mismatched_hash = super::sha(b"unit-only mismatched fixture hash");
    replace_plan_fixture_hash(&inputs.plan_path, &mismatched_hash)?;
    let result = run_worker(&inputs)?;
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr)
        .contains("operation plan fixture SHA does not match input"));
    Ok(())
}

#[test]
fn cli_rejects_missing_and_invalid_u32_and_f32_arguments() -> Result<(), Box<dyn std::error::Error>>
{
    for (numbers, expected_error) in option_error_cases() {
        let numeric_args = numbers.iter().map(String::as_str).collect::<Vec<_>>();
        let result = run_cli_args(&option_args(&numeric_args))?;
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(!result.status.success(), "{expected_error}: {stderr}");
        assert!(
            stderr.contains(expected_error),
            "{expected_error}: {stderr}"
        );
    }
    Ok(())
}
