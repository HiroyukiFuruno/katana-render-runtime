#![cfg(test)]

use super::{test_support::case_with_static_report, validate_sample_input};
use crate::options::OperationPlan;
use crate::provenance::hex_digest;
use std::{fs, io};

#[test]
fn accepts_catalog_evidence_path_with_current_directory_component(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut case = case_with_static_report()?;
    if let OperationPlan::Catalog(plan) = &mut case.plan {
        let parent = case
            .report_path
            .parent()
            .ok_or("report path must have a parent")?;
        plan.operation_evidence_path = format!("{}/./catalog-report.json", parent.display()).into();
    }
    assert!(!validate_sample_input(
        &case.plan,
        &case.source_hash,
        case.viewport
    )?);
    Ok(())
}

#[cfg(unix)]
#[test]
fn rejects_catalog_evidence_path_with_symlinked_parent_directory(
) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let mut case = case_with_static_report()?;
    let parent = case
        .report_path
        .parent()
        .ok_or("report path must have a parent")?;
    let alias_directory = parent.join("catalog-report-directory-alias");
    symlink(parent, &alias_directory)?;
    if let OperationPlan::Catalog(plan) = &mut case.plan {
        plan.operation_evidence_path = alias_directory.join("catalog-report.json");
    }
    let error = validate_sample_input(&case.plan, &case.source_hash, case.viewport)
        .err()
        .ok_or("symlinked parent directory must be rejected")?;
    assert_eq!(
        error.to_string(),
        "catalog operation evidence path must be a physical file"
    );
    Ok(())
}

#[test]
fn rejects_missing_catalog_evidence_file_at_canonicalize_boundary(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut case = case_with_static_report()?;
    if let OperationPlan::Catalog(plan) = &mut case.plan {
        let parent = case
            .report_path
            .parent()
            .ok_or("report path must have a parent")?;
        plan.operation_evidence_path = parent.join("missing-catalog-report.json");
    }
    let error = validate_sample_input(&case.plan, &case.source_hash, case.viewport)
        .err()
        .ok_or("missing catalog evidence file must be rejected")?;
    assert_eq!(
        error.downcast_ref::<io::Error>().map(io::Error::kind),
        Some(io::ErrorKind::NotFound)
    );
    Ok(())
}

#[test]
fn rejects_directory_at_catalog_evidence_read_boundary() -> Result<(), Box<dyn std::error::Error>> {
    let mut case = case_with_static_report()?;
    let directory = case
        .report_path
        .parent()
        .ok_or("report path must have a parent")?
        .to_path_buf();
    let expected_error = fs::read(&directory)
        .err()
        .ok_or("directory read unexpectedly succeeded")?;
    if let OperationPlan::Catalog(plan) = &mut case.plan {
        plan.operation_evidence_path = directory;
    }
    let error = validate_sample_input(&case.plan, &case.source_hash, case.viewport)
        .err()
        .ok_or("directory evidence path must be rejected")?;
    assert_eq!(
        error.downcast_ref::<io::Error>().map(io::Error::kind),
        Some(expected_error.kind())
    );
    Ok(())
}

#[test]
fn rejects_invalid_json_after_matching_evidence_sha() -> Result<(), Box<dyn std::error::Error>> {
    let mut case = case_with_static_report()?;
    let bytes = b"{invalid json";
    fs::write(&case.report_path, bytes)?;
    if let OperationPlan::Catalog(plan) = &mut case.plan {
        plan.operation_evidence_sha256 = hex_digest(bytes);
    }
    let error = validate_sample_input(&case.plan, &case.source_hash, case.viewport)
        .err()
        .ok_or("invalid pinned JSON must be rejected")?;
    assert!(error.downcast_ref::<serde_json::Error>().is_some());
    Ok(())
}
