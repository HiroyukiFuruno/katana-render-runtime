use super::super::{
    MachoNoso, canonical_file, exact_keys, input_rows, inspect_file, is_sha256, sha256,
};
use super::{
    invalid_path_inputs, manifest, write_fixture, write_malformed_manifest, write_verified_manifest,
};
use serde_json::{Value, json};

const EXPECTED_NOSO_RECORDS: usize = 1;
const SIZE_DRIFT_BYTES: usize = 1;
const INVALID_MANIFEST_COUNT: usize = 4;

#[test]
fn input_rows_accepts_canonical_file_and_rejects_schema_shapes() -> Result<(), String> {
    let fixture = write_fixture()?;
    let parsed = input_rows(&manifest(
        &fixture.path,
        fixture.bytes.len(),
        &fixture.digest,
    ))?;
    assert_eq!(parsed.len(), 1);
    let invalid = invalid_manifests(&fixture);
    for value in invalid {
        assert!(input_rows(&value).is_err());
    }
    Ok(())
}

fn invalid_manifests(fixture: &super::Fixture) -> [Value; INVALID_MANIFEST_COUNT] {
    [
        json!({"schema":"wrong","files":[]}),
        json!({"schema":"krr-macho-noso-input/v1","files":[]}),
        json!({"schema":"krr-macho-noso-input/v1","files":[{"path":fixture.path.to_string_lossy(),"size":fixture.bytes.len(),"sha256":"ABC"}]}),
        json!({"schema":"krr-macho-noso-input/v1","files":[{"path":fixture.path.to_string_lossy(),"size":fixture.bytes.len(),"sha256":fixture.digest,"extra":true}]}),
    ]
}

#[test]
fn exact_keys_and_canonical_file_reject_invalid_inputs() -> Result<(), String> {
    assert!(exact_keys(&Value::Null, &["path"], "row").is_err());
    assert!(exact_keys(&json!({"path":"x","extra":true}), &["path"], "row").is_err());
    let (directory, missing) = invalid_path_inputs()?;
    assert!(canonical_file(directory.path()).is_err());
    assert!(canonical_file(&missing).is_err());
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
    assert_verified_manifest_runs(&fixture)?;
    assert_malformed_manifest_is_rejected(&fixture)?;
    assert!(MachoNoso::records(b"not-mach-o").is_err());
    Ok(())
}

fn assert_verified_manifest_runs(fixture: &super::Fixture) -> Result<(), String> {
    let manifest_path = write_verified_manifest(fixture)?;
    MachoNoso::run(&manifest_path)
}

fn assert_malformed_manifest_is_rejected(fixture: &super::Fixture) -> Result<(), String> {
    let malformed_path = write_malformed_manifest(fixture)?;
    assert!(MachoNoso::run(&malformed_path).is_err());
    Ok(())
}
