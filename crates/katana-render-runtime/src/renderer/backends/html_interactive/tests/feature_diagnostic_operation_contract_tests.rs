use super::{CatalogOperation, FeatureRequest, TestResult, validate_operation_request};
use serde_json::{Value, json};

#[test]
fn rejects_mismatched_catalog_operation_identity() -> TestResult {
    let request = fixture_request();
    validate_operation_request(&fixture_operation(&[])?, &request)?;
    let cases = [
        (
            "schema",
            json!("krr-html84-operation-v2"),
            "unsupported catalog operation schema",
        ),
        (
            "viewport",
            json!({"width": 1280, "height": 900, "scale": 2}),
            "catalog operation viewport differs from feature environment",
        ),
        (
            "conditionId",
            json!("typography/ja-en-mixed/960x720@1"),
            "catalog condition identity does not match its case, state, and viewport",
        ),
    ];
    assert_rejected_operations(&request, cases)?;
    let mut other_case = request;
    other_case.case_id = "layout".into();
    assert_rejected(
        &fixture_operation(&[])?,
        &other_case,
        "catalog operation case differs from legacy case environment",
    )
}

#[test]
fn rejects_catalog_operation_outside_the_fixed_inventory() -> TestResult {
    let request = fixture_request();
    let cases = [
        (
            "targetSelector",
            json!("#different"),
            "catalog operation target differs from the finite catalog",
        ),
        (
            "operationKind",
            json!("script-load-observe"),
            "unsupported or mismatched catalog operation",
        ),
    ];
    assert_rejected_operations(&request, cases)?;
    let mut other_viewport = request;
    other_viewport.width = 960;
    let operation = fixture_operation(&[
        ("conditionId", json!("typography/ja-en-mixed/960x900@1")),
        ("viewport", json!({"width": 960, "height": 900, "scale": 1})),
    ])?;
    assert_rejected(
        &operation,
        &other_viewport,
        "catalog viewport is outside the finite inventory",
    )
}

#[test]
fn rejects_unknown_boundary_state_for_scripted_catalog_case() -> TestResult {
    let mut request = fixture_request();
    request.case_id = "dynamic-dom".into();
    let operation = fixture_operation(&[
        ("caseId", json!("dynamic-dom")),
        ("boundaryState", json!("unknown")),
        ("conditionId", json!("dynamic-dom/unknown/1280x900@1")),
        ("operationKind", json!("script-load-observe")),
        ("targetSelector", Value::Null),
    ])?;
    assert_rejected(
        &operation,
        &request,
        "unknown catalog case or boundary state",
    )
}

fn assert_rejected_operations(
    request: &FeatureRequest,
    cases: impl IntoIterator<Item = (&'static str, Value, &'static str)>,
) -> TestResult {
    for (field, replacement, expected) in cases {
        assert_rejected(
            &fixture_operation(&[(field, replacement)])?,
            request,
            expected,
        )?;
    }
    Ok(())
}

fn assert_rejected(
    operation: &CatalogOperation,
    request: &FeatureRequest,
    expected: &str,
) -> TestResult {
    let error = validate_operation_request(operation, request)
        .err()
        .ok_or("mismatched operation was accepted")?;
    assert_eq!(error.to_string(), expected);
    Ok(())
}

fn fixture_operation(replacements: &[(&str, Value)]) -> TestResult<CatalogOperation> {
    let mut operation = json!({
        "schema": "krr-html84-operation-v1",
        "conditionId": "typography/ja-en-mixed/1280x900@1",
        "caseId": "typography",
        "boundaryState": "ja-en-mixed",
        "operationKind": "load-and-observe-target",
        "targetSelector": ".mixed",
        "viewport": {"width": 1280, "height": 900, "scale": 1},
    });
    for (field, value) in replacements {
        operation[*field] = value.clone();
    }
    serde_json::from_value(operation).map_err(|error| error.to_string())
}

fn fixture_request() -> FeatureRequest {
    FeatureRequest {
        input: Default::default(),
        source_sha256: String::new(),
        expression: String::new(),
        case_id: "typography".into(),
        nonce: String::new(),
        binary_sha: String::new(),
        output: Default::default(),
        width: 1280,
        height: 900,
        operation: None,
    }
}
