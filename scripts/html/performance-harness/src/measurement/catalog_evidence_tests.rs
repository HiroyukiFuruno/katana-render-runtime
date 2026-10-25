#![cfg(test)]

use super::{catalog_condition_report, validate_sample_input};
use crate::{
    options::{CatalogInput, OperationPlan},
    test_support::TestSupport,
};
use serde_json::json;
use std::fs;

#[path = "catalog_evidence_test_support.rs"]
mod support;
use support::{
    case_with_mutated_static_report, case_with_report, case_with_static_report,
    fixture_source_hash, operation_report, settled_report, Mutation, STATIC_OPERATION_KIND,
};

#[test]
fn catalog_condition_report_emits_operation_identity() -> Result<(), Box<dyn std::error::Error>> {
    let plan = serde_json::from_value::<OperationPlan>(TestSupport::catalog_test_plan())?;
    let report = catalog_condition_report(&plan);
    assert_eq!(report["profile"], "catalog-plan84");
    assert_eq!(report["condition_id"], "async/expected-state/1280x900@1");
    assert_eq!(report["case_id"], "async");
    assert_eq!(report["state"], "expected-state");
    assert_eq!(report["operation_kind"], "script-await-timer-microtask");
    assert_eq!(report["operation_evidence_sha256"], "b".repeat(64));
    Ok(())
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
fn rejects_evidence_changed_after_plan_hash_was_pinned() -> Result<(), Box<dyn std::error::Error>> {
    let case = case_with_static_report()?;
    fs::write(&case.report_path, b"tampered after the plan was written")?;
    assert!(validate_sample_input(&case.plan, &case.source_hash, case.viewport).is_err());
    Ok(())
}

#[test]
fn rejects_plan_when_fixture_sha_does_not_match_input() -> Result<(), Box<dyn std::error::Error>> {
    let mut case = case_with_static_report()?;
    if let OperationPlan::Catalog(plan) = &mut case.plan {
        plan.fixture_sha256 = "0".repeat(64);
    }
    assert!(validate_sample_input(&case.plan, &case.source_hash, case.viewport).is_err());
    Ok(())
}

#[test]
fn rejects_invalid_json_in_a_correctly_pinned_report_file() -> Result<(), Box<dyn std::error::Error>>
{
    let mut case = case_with_static_report()?;
    let bytes = b"{invalid json";
    fs::write(&case.report_path, bytes)?;
    if let OperationPlan::Catalog(plan) = &mut case.plan {
        plan.operation_evidence_sha256 = crate::provenance::hex_digest(bytes);
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

#[test]
fn validates_settled_catalog_operation_kinds_through_sample_input(
) -> Result<(), Box<dyn std::error::Error>> {
    let source_hash = fixture_source_hash()?;
    for (kind, step) in [
        (
            "script-await-timer-microtask",
            json!({
                "kind": "host-time-advance", "elapsedMs": 0, "timerRan": false,
                "microtaskCheckpoint": "runtime-advance-timers", "stateBefore": "ready",
                "stateAfter": "ready", "fixtureStateVerified": true
            }),
        ),
        (
            "script-await-resource-font",
            json!({
                "kind": "session-load-settled-resource-state",
                "stateBefore": "ready", "stateAfter": "ready"
            }),
        ),
    ] {
        let report = settled_report(&source_hash, kind, step);
        let case = case_with_report(report, &source_hash, kind)?;
        assert!(validate_sample_input(
            &case.plan,
            &case.source_hash,
            case.viewport
        )?);
    }
    Ok(())
}

#[test]
fn rejects_catalog_streams_inconsistent_with_operation_kind(
) -> Result<(), Box<dyn std::error::Error>> {
    let source_hash = fixture_source_hash()?;
    for (kind, has_stream) in [(STATIC_OPERATION_KIND, true), ("script-real-events", false)] {
        let mut case = case_with_report(operation_report(&source_hash, kind), &source_hash, kind)?;
        if let OperationPlan::Catalog(plan) = &mut case.plan {
            plan.operation_stream = stream_for(has_stream);
        }
        let error = validate_sample_input(&case.plan, &case.source_hash, case.viewport)
            .err()
            .ok_or("operation kind and stream mismatch must be rejected")?;
        assert_eq!(
            error.to_string(),
            "catalog operation stream does not match its finite operation kind",
            "kind={kind}, has_stream={has_stream}"
        );
    }
    Ok(())
}

fn stream_for(has_stream: bool) -> Vec<CatalogInput> {
    if has_stream {
        vec![CatalogInput::Scroll {
            delta_x: 0.0,
            delta_y: 1.0,
        }]
    } else {
        Vec::new()
    }
}

#[test]
fn rejects_non_object_catalog_viewport() -> Result<(), Box<dyn std::error::Error>> {
    let source_hash = fixture_source_hash()?;
    let mut report = operation_report(&source_hash, STATIC_OPERATION_KIND);
    report["viewport"] = json!("not-an-object");
    let case = case_with_report(report, &source_hash, STATIC_OPERATION_KIND)?;
    let error = validate_sample_input(&case.plan, &case.source_hash, case.viewport)
        .err()
        .ok_or("non-object viewport must be rejected")?;
    assert_eq!(
        error.to_string(),
        "catalog operation evidence status or identity differs"
    );
    Ok(())
}
