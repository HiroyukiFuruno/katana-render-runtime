use super::{MachoNoso, canonical_file, exact_keys, input_rows, inspect_file, is_sha256, sha256};
use crate::macho_noso_test_support::{Corruption, MachoNosoFixture};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

const EXPECTED_NOSO_RECORDS: usize = 1;
const SIZE_DRIFT_BYTES: usize = 1;

struct Fixture {
    _directory: tempfile::TempDir,
    path: std::path::PathBuf,
    bytes: Vec<u8>,
    digest: String,
}

fn write_fixture() -> Result<Fixture, String> {
    let directory = tempdir().map_err(|error| error.to_string())?;
    let bytes = MachoNosoFixture::thin_macho(true, Corruption::None);
    let path = directory.path().join("fixture.o");
    fs::write(&path, &bytes).map_err(|error| error.to_string())?;
    let path = path.canonicalize().map_err(|error| error.to_string())?;
    Ok(Fixture {
        _directory: directory,
        path,
        digest: sha256(&bytes),
        bytes,
    })
}

fn manifest(path: &Path, size: usize, digest: &str) -> Value {
    json!({
        "schema": "krr-macho-noso-input/v1",
        "files": [{"path": path.to_string_lossy(), "size": size, "sha256": digest}]
    })
}

#[test]
fn input_rows_accepts_canonical_file_and_rejects_schema_shapes() -> Result<(), String> {
    let fixture = write_fixture()?;
    let parsed = input_rows(&manifest(
        &fixture.path,
        fixture.bytes.len(),
        &fixture.digest,
    ))?;
    assert_eq!(parsed.len(), 1);
    let invalid = [
        json!({"schema":"wrong","files":[]}),
        json!({"schema":"krr-macho-noso-input/v1","files":[]}),
        json!({"schema":"krr-macho-noso-input/v1","files":[{"path":fixture.path.to_string_lossy(),"size":fixture.bytes.len(),"sha256":"ABC"}]}),
        json!({"schema":"krr-macho-noso-input/v1","files":[{"path":fixture.path.to_string_lossy(),"size":fixture.bytes.len(),"sha256":fixture.digest,"extra":true}]}),
    ];
    for value in invalid {
        assert!(input_rows(&value).is_err());
    }
    Ok(())
}

#[test]
fn exact_keys_and_canonical_file_reject_invalid_inputs() -> Result<(), String> {
    assert!(exact_keys(&Value::Null, &["path"], "row").is_err());
    assert!(exact_keys(&json!({"path":"x","extra":true}), &["path"], "row").is_err());
    let directory = tempdir().map_err(|error| error.to_string())?;
    assert!(canonical_file(directory.path()).is_err());
    assert!(canonical_file(&directory.path().join("missing")).is_err());
    Ok(())
}

#[test]
fn inspect_file_accepts_verified_macho() -> Result<(), String> {
    let fixture = write_fixture()?;
    let output = inspect_file(
        fixture.path.clone(),
        fixture.bytes.len() as u64,
        &fixture.digest,
    )?;
    assert_eq!(output["format"], "MachO64");
    assert_eq!(
        output["nOso"].as_array().map(Vec::len),
        Some(EXPECTED_NOSO_RECORDS)
    );
    assert!(is_sha256(&fixture.digest));
    Ok(())
}

#[test]
fn inspect_file_rejects_size_or_digest_drift() -> Result<(), String> {
    let fixture = write_fixture()?;
    assert!(
        inspect_file(
            fixture.path.clone(),
            (fixture.bytes.len() + SIZE_DRIFT_BYTES) as u64,
            &fixture.digest
        )
        .is_err()
    );
    assert!(
        inspect_file(
            fixture.path,
            fixture.bytes.len() as u64,
            &sha256(b"different")
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn run_rejects_noncanonical_manifest_and_accepts_verified_fixture() -> Result<(), String> {
    let fixture = write_fixture()?;
    let manifest_path = fixture.path.with_file_name("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest(
            &fixture.path,
            fixture.bytes.len(),
            &fixture.digest,
        ))
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    MachoNoso::run(&manifest_path)?;

    let malformed_path = fixture.path.with_file_name("malformed.json");
    fs::write(&malformed_path, b"[").map_err(|e| e.to_string())?;
    assert!(MachoNoso::run(&malformed_path).is_err());
    assert!(MachoNoso::records(b"not-mach-o").is_err());
    Ok(())
}
