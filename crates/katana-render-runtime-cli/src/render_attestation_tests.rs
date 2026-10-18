use super::{AttestationPaths, RenderAttestation};
use crate::commands::DiagramInputFormat;
use crate::diagram_cmd::RenderInputFactory;
use katana_render_runtime::{
    DiagramKind, RenderDiagnostics, RenderOutput, RendererProfile, RuntimeVersion,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    root: PathBuf,
    input: PathBuf,
    runtime: PathBuf,
    output: PathBuf,
    receipt: PathBuf,
}

impl Fixture {
    fn new(source: &str) -> anyhow::Result<Self> {
        let parent = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tmp/html-fidelity/frame-capacity-independent-review-r1/native-input-svg-attestation-implementation-r1/unit-artifacts");
        fs::create_dir_all(&parent)?;
        let parent = fs::canonicalize(parent)?;
        let root = parent.join(format!(
            "{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root)?;
        let fixture = Self {
            input: root.join("input.mmd"),
            runtime: root.join("runtime.js"),
            output: root.join("output.svg"),
            receipt: root.join("attestation.json"),
            root,
        };
        fs::write(&fixture.input, source)?;
        fs::write(&fixture.runtime, b"unit contract runtime placeholder")?;
        Ok(fixture)
    }

    fn begin(&self, source: &str) -> anyhow::Result<RenderAttestation> {
        self.begin_with_format(source, DiagramInputFormat::DiagramSource, "contract-test")
    }

    fn begin_with_captured_input(
        &self,
        source: &str,
    ) -> anyhow::Result<(RenderAttestation, katana_render_runtime::RenderInput)> {
        let attestation = self.begin(source)?;
        let input = RenderInputFactory::create(
            DiagramKind::Mermaid,
            source.into(),
            serde_json::Value::Null,
        );
        attestation.capture_input(&input)?;
        Ok((attestation, input))
    }

    fn begin_with_format(
        &self,
        source: &str,
        format: DiagramInputFormat,
        token: &str,
    ) -> anyhow::Result<RenderAttestation> {
        RenderAttestation::begin(
            AttestationPaths {
                input: &self.input,
                output: Some(&self.output),
                runtime: &self.runtime,
                receipt: &self.receipt,
            },
            Some(token),
            format,
            source.as_bytes(),
        )
    }
}

fn unit_output() -> RenderOutput {
    RenderOutput {
        svg: "<svg xmlns=\"http://www.w3.org/2000/svg\"><text>unit contract output</text></svg>"
            .into(),
        width: 1.0,
        height: 1.0,
        view_box: "0 0 1 1".into(),
        runtime: RuntimeVersion {
            name: "unit-placeholder".into(),
            version: "0".into(),
            checksum: None,
        },
        profile: RendererProfile {
            id: "unit-contract".into(),
            description: None,
        },
        diagnostics: RenderDiagnostics {
            warnings: Vec::new(),
            errors: Vec::new(),
        },
        cache_fingerprint: "unit-contract".into(),
    }
}

#[test]
fn receipt_binds_actual_input_output_files_and_needs_external_exit() -> anyhow::Result<()> {
    let source = "\nclassDiagram\r\n class 動物  \r\n";
    let fixture = Fixture::new(source)?;
    let (attestation, input) = fixture.begin_with_captured_input(source)?;
    attestation.complete(&input, &unit_output())?;
    let receipt: serde_json::Value = serde_json::from_slice(&fs::read(&fixture.receipt)?)?;
    assert_receipt_identity(&receipt, source);
    assert_artifact_hashes(&receipt)?;
    assert_eq!(fs::read(&fixture.output)?, unit_output().svg.as_bytes());
    assert_eq!(
        fs::read(
            fixture
                .receipt
                .with_extension("artifacts")
                .join("raw-input.bin")
        )?,
        source.as_bytes()
    );
    Ok(())
}

fn assert_receipt_identity(receipt: &serde_json::Value, source: &str) {
    assert_eq!(receipt["schema"], "katana-native-render-attestation-v1");
    assert_eq!(receipt["actualRenderInput"]["source"], source);
    assert_eq!(
        receipt["actualRenderInput"]["config"]["vendor_config"],
        serde_json::Value::Null
    );
    assert_eq!(receipt["pid"], std::process::id());
    assert_eq!(receipt["attemptToken"], "contract-test");
    assert_eq!(receipt["actualExit"], serde_json::Value::Null);
    assert_eq!(receipt["accepted"], false);
    assert_eq!(receipt["requiresExternalExitEvidence"], true);
    assert_eq!(receipt["bindingsBefore"], receipt["bindingsAfter"]);
}

fn assert_artifact_hashes(receipt: &serde_json::Value) -> anyhow::Result<()> {
    for key in ["rawInput", "renderSource", "renderInput", "output"] {
        let record = &receipt["artifacts"][key];
        let path = record["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing artifact path"))?;
        assert_eq!(record["sha256"], super::sha256(&fs::read(path)?));
    }
    Ok(())
}

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
    assert!(super::FileRecord::read(std::path::Path::new("relative-input.mmd")).is_err());
    assert!(super::validate_destination(std::path::Path::new("relative-output.svg")).is_err());
    Ok(())
}

#[test]
fn sha256_matches_standard_known_answer() {
    assert_eq!(
        super::sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[path = "render_attestation_option_tests.rs"]
mod option_tests;

#[test]
fn file_record_rejects_non_regular_and_noncanonical_paths() -> anyhow::Result<()> {
    let fixture = Fixture::new("path validation")?;
    let directory_error = match super::FileRecord::read(&fixture.root) {
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
    let path_error = match super::FileRecord::read(&noncanonical) {
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
    let metadata_error = match super::validate_destination(&oversized_name) {
        Err(error) => error,
        Ok(()) => anyhow::bail!("overlong destination name was accepted"),
    };
    assert!(metadata_error.downcast_ref::<std::io::Error>().is_some());

    let artifacts = super::create_artifacts(&fixture.receipt, b"raw source")?;
    assert_eq!(fs::read(artifacts.join("raw-input.bin"))?, b"raw source");
    let duplicate_error = match super::create_artifacts(&fixture.receipt, b"replacement") {
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
