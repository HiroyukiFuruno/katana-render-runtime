use super::super::{FileRecord, create_artifacts, validate_destination};
use super::{AttestationPaths, Fixture, RenderAttestation, unit_output};
use crate::commands::DiagramInputFormat;
use std::{fs, path::Path};

#[test]
fn stale_read_and_existing_destinations_fail_without_overwrite() -> anyhow::Result<()> {
    let fixture = Fixture::new("original")?;
    assert!(fixture.begin("different").is_err());
    fs::write(&fixture.output, b"protected old output")?;
    assert!(fixture.begin("original").is_err());
    assert_eq!(fs::read(&fixture.output)?, b"protected old output");
    assert!(!fixture.receipt.exists());
    Ok(())
}

#[test]
fn runtime_drift_prevents_output_and_success_receipt() -> anyhow::Result<()> {
    let fixture = Fixture::new("classDiagram")?;
    let (attestation, input) = fixture.begin_with_captured_input("classDiagram")?;
    fs::write(&fixture.runtime, b"different unit runtime")?;
    assert!(attestation.complete(&input, &unit_output()).is_err());
    assert!(!fixture.output.exists());
    assert!(!fixture.receipt.exists());
    assert!(fixture.receipt.with_extension("artifacts").exists());
    Ok(())
}

#[test]
fn changed_render_input_snapshot_is_rejected() -> anyhow::Result<()> {
    let fixture = Fixture::new("classDiagram")?;
    let (attestation, input) = fixture.begin_with_captured_input("classDiagram")?;
    fs::write(
        fixture
            .receipt
            .with_extension("artifacts")
            .join("request-source.mmd"),
        b"old unrelated request",
    )?;
    assert!(attestation.complete(&input, &unit_output()).is_err());
    assert!(!fixture.output.exists());
    assert!(!fixture.receipt.exists());
    Ok(())
}

#[test]
fn receipt_write_failure_retains_output_and_cannot_claim_completion() -> anyhow::Result<()> {
    let fixture = Fixture::new("classDiagram")?;
    let (attestation, input) = fixture.begin_with_captured_input("classDiagram")?;
    fs::write(&fixture.receipt, b"protected old receipt")?;
    assert!(attestation.complete(&input, &unit_output()).is_err());
    assert_eq!(fs::read(&fixture.receipt)?, b"protected old receipt");
    assert!(fixture.output.is_file());
    assert!(fixture.root.is_dir());
    Ok(())
}

#[test]
fn invalid_token_and_relative_paths_are_rejected() -> anyhow::Result<()> {
    let fixture = Fixture::new("classDiagram")?;
    let result = RenderAttestation::begin(
        AttestationPaths {
            input: &fixture.input,
            output: Some(&fixture.output),
            runtime: &fixture.runtime,
            receipt: &fixture.receipt,
        },
        Some("../other-attempt"),
        DiagramInputFormat::DiagramSource,
        b"classDiagram",
    );
    assert!(result.is_err());
    assert!(FileRecord::read(Path::new("relative-input.mmd")).is_err());
    assert!(validate_destination(Path::new("relative-output.svg")).is_err());
    Ok(())
}

#[test]
fn missing_receipt_parent_rejects_attestation_before_creating_output() -> anyhow::Result<()> {
    let fixture = Fixture::new("classDiagram")?;
    let receipt = fixture.root.join("missing-parent").join("receipt.json");
    let result = RenderAttestation::begin(
        AttestationPaths {
            input: &fixture.input,
            output: Some(&fixture.output),
            runtime: &fixture.runtime,
            receipt: &receipt,
        },
        Some("contract-test"),
        DiagramInputFormat::DiagramSource,
        b"classDiagram",
    );
    let error = match result {
        Err(error) => error,
        Ok(_) => anyhow::bail!("missing receipt parent was accepted"),
    };
    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .map(|error| error.kind()),
        Some(std::io::ErrorKind::NotFound)
    );
    assert!(!fixture.output.exists());
    assert!(!receipt.exists());
    assert!(!receipt.with_extension("artifacts").exists());
    Ok(())
}

#[test]
fn attestation_requires_output_path() -> anyhow::Result<()> {
    let fixture = Fixture::new("classDiagram")?;
    let error = match RenderAttestation::begin(
        AttestationPaths {
            input: &fixture.input,
            output: None,
            runtime: &fixture.runtime,
            receipt: &fixture.receipt,
        },
        Some("contract-test"),
        DiagramInputFormat::DiagramSource,
        b"classDiagram",
    ) {
        Err(error) => error,
        Ok(_) => anyhow::bail!("attestation without output path was accepted"),
    };

    assert!(
        error
            .to_string()
            .contains("render attestation requires an output file")
    );
    assert!(!fixture.receipt.exists());
    Ok(())
}

#[test]
fn file_record_rejects_non_regular_and_noncanonical_paths() -> anyhow::Result<()> {
    let fixture = Fixture::new("path validation")?;
    let directory_error = match FileRecord::read(&fixture.root) {
        Err(error) => error,
        Ok(_) => anyhow::bail!("directory accepted as regular file"),
    };
    assert!(
        directory_error
            .to_string()
            .contains("attestation requires regular files")
    );

    let child = fixture.root.join("child");
    fs::create_dir(&child)?;
    let noncanonical = child.join("..").join("input.mmd");
    assert!(noncanonical.is_absolute());
    assert_ne!(fs::canonicalize(&noncanonical)?, noncanonical);
    let path_error = match FileRecord::read(&noncanonical) {
        Err(error) => error,
        Ok(_) => anyhow::bail!("noncanonical file path was accepted"),
    };
    assert!(
        path_error
            .to_string()
            .contains("attestation path must be canonical")
    );
    Ok(())
}

#[test]
fn destination_propagates_other_metadata_errors_and_artifact_directory_must_be_fresh()
-> anyhow::Result<()> {
    let fixture = Fixture::new("destination validation")?;
    let oversized_name = fixture.root.join("x".repeat(300));
    let metadata_error = match validate_destination(&oversized_name) {
        Err(error) => error,
        Ok(()) => anyhow::bail!("overlong destination name was accepted"),
    };
    assert!(metadata_error.downcast_ref::<std::io::Error>().is_some());

    let artifacts = create_artifacts(&fixture.receipt, b"raw source")?;
    assert_eq!(fs::read(artifacts.join("raw-input.bin"))?, b"raw source");
    let duplicate_error = match create_artifacts(&fixture.receipt, b"replacement") {
        Err(error) => error,
        Ok(_) => anyhow::bail!("existing artifact directory was accepted"),
    };
    assert!(
        duplicate_error
            .to_string()
            .contains("attestation artifacts must be fresh")
    );
    assert_eq!(fs::read(artifacts.join("raw-input.bin"))?, b"raw source");
    Ok(())
}
