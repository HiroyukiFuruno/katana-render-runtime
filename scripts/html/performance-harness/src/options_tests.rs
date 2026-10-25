#![cfg(test)]

use super::{parse_options, worker_sample};
use super::{LegacyOperationPlan, OperationPlan};
use serde_json::json;
use std::{ffi::OsString, fs, iter, path::PathBuf};

fn parse_catalog_plan(
    plan: serde_json::Value,
    trailing: bool,
) -> Result<super::Options, Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let plan_path = directory.path().join("plan.json");
    fs::write(&plan_path, serde_json::to_vec(&plan)?)?;
    let mut args = vec![
        OsString::from("input.html"),
        OsString::from("result.json"),
        plan_path.into_os_string(),
        OsString::from("1280"),
        OsString::from("900"),
        OsString::from("1"),
    ];
    if trailing {
        args.push(OsString::from("unexpected"));
    }
    parse_options(args.into_iter())
}

fn legacy_plan() -> OperationPlan {
    OperationPlan::Legacy(LegacyOperationPlan {
        fixture_sha256: "legacy".into(),
        operation: "click".into(),
        x: 1.0,
        y: 2.0,
        expected_dom_state_path: PathBuf::from("state.json"),
        expected_dom_state_sha256: "state".into(),
    })
}

fn catalog_model() -> Result<OperationPlan, Box<dyn std::error::Error>> {
    Ok(OperationPlan::Catalog(serde_json::from_value(
        crate::test_support::TestSupport::catalog_test_plan(),
    )?))
}

#[test]
fn operation_plan_accessors_preserve_legacy_and_catalog_contracts(
) -> Result<(), Box<dyn std::error::Error>> {
    let legacy = legacy_plan();
    let catalog = catalog_model()?;
    assert_eq!(legacy.fixture_sha256(), "legacy");
    assert_eq!(legacy.operation(), "click");
    assert_eq!(legacy.expected_dom_state_sha256(), "state");
    assert_eq!(catalog.fixture_sha256(), "a".repeat(64));
    assert_eq!(catalog.operation(), "script-await-timer-microtask");
    assert_eq!(catalog.expected_dom_state_sha256(), "");
    Ok(())
}

#[test]
fn parse_options_accepts_catalog_and_rejects_bounded_drift(
) -> Result<(), Box<dyn std::error::Error>> {
    let parsed = parse_catalog_plan(crate::test_support::TestSupport::catalog_test_plan(), false)?;
    assert_eq!(
        parsed.operation_plan.operation(),
        "script-await-timer-microtask"
    );
    let trailing = parse_error(crate::test_support::TestSupport::catalog_test_plan(), true)?;
    assert_eq!(trailing, "unexpected trailing argument");
    for (label, field, value) in [
        ("profile", "profile", json!("wrong-profile")),
        ("identity", "conditionId", json!("")),
        (
            "viewport",
            "viewport",
            json!({"width": 640, "height": 900, "scale": 1.0}),
        ),
    ] {
        assert_catalog_drift_rejected(label, field, value)?;
    }
    Ok(())
}

fn assert_catalog_drift_rejected(
    label: &str,
    field: &str,
    value: serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut plan = crate::test_support::TestSupport::catalog_test_plan();
    plan[field] = value;
    let error = parse_error(plan, false)?;
    assert_eq!(
        error, "catalog operation plan does not match the worker viewport",
        "{label}"
    );
    Ok(())
}

fn parse_error(
    plan: serde_json::Value,
    trailing: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    match parse_catalog_plan(plan, trailing) {
        Ok(_) => Err("invalid catalog plan was accepted".into()),
        Err(error) => Ok(error.to_string()),
    }
}

#[test]
fn normal_invocation_preserves_first_input_argument() -> Result<(), Box<dyn std::error::Error>> {
    let input = OsString::from("fixture.html");
    let mut args = vec![
        input.clone(),
        OsString::from("result.json"),
        OsString::from("plan.json"),
    ]
    .into_iter()
    .peekable();
    assert_eq!(worker_sample(&mut args)?, None);
    assert_eq!(args.next(), Some(input));
    Ok(())
}

#[test]
fn worker_invocation_consumes_only_marker_and_sample() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec![
        OsString::from("--worker"),
        OsString::from("4"),
        OsString::from("fixture.html"),
    ]
    .into_iter()
    .peekable();
    assert_eq!(worker_sample(&mut args)?, Some(4));
    assert_eq!(args.next(), Some(OsString::from("fixture.html")));
    Ok(())
}

#[test]
fn worker_marker_requires_a_sample() {
    let mut args = iter::once(OsString::from("--worker")).peekable();
    assert!(worker_sample(&mut args).is_err());
}
