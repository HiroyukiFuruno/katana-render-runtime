use super::super::{
    MachoNoso, canonical_file, exact_keys, input_rows, inspect_file, is_sha256, sha256,
    validate_read,
};
use super::{
    invalid_path_inputs, manifest, write_fixture, write_malformed_manifest, write_verified_manifest,
};
use serde_json::{Value, json};

const SIZE_DRIFT_BYTES: usize = 1;
const INVALID_MANIFEST_COUNT: usize = 4;
const EXAMPLE_SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn read_validation_rejects_metadata_drift_and_incomplete_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    use std::{
        fs,
        time::{Duration, UNIX_EPOCH},
    };

    let fixture = write_fixture()?;
    let file = fs::OpenOptions::new().write(true).open(&fixture.path)?;
    let before = file.metadata()?;
    let size = fixture.bytes.len();
    assert!(validate_read(&before, &before, size, before.len()).is_ok());
    file.set_modified(UNIX_EPOCH + Duration::from_secs(1_000_000_000))?;
    let time_changed = file.metadata()?;
    assert_eq!(before.len(), time_changed.len());
    assert_ne!(before.modified()?, time_changed.modified()?);
    file.set_len(before.len() + 1)?;
    let size_changed = file.metadata()?;
    for (after, bytes_read) in [
        (&time_changed, size),
        (&size_changed, size),
        (&before, size - 1),
    ] {
        assert_changed_read(&before, after, bytes_read);
    }
    Ok(())
}

fn assert_changed_read(before: &std::fs::Metadata, after: &std::fs::Metadata, bytes_read: usize) {
    assert_eq!(
        validate_read(before, after, bytes_read, before.len())
            .err()
            .as_deref(),
        Some("input-file:changed-during-read")
    );
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
    let invalid = invalid_manifests(&fixture);
    for value in invalid {
        assert!(input_rows(&value).is_err());
    }
    Ok(())
}

#[test]
fn input_rows_rejects_wrong_json_shapes_and_field_types() -> Result<(), String> {
    let fixture = write_fixture()?;
    let path = fixture.path.to_string_lossy();
    for (manifest, expected) in [
        (Value::Null, "input-manifest:object"),
        (
            json!({"schema": "krr-macho-noso-input/v1"}),
            "input-manifest:keys",
        ),
        (
            json!({"schema": "krr-macho-noso-input/v1", "files": {}}),
            "input-manifest:files",
        ),
        (
            json!({"schema": "krr-macho-noso-input/v1", "files": [{"path": null, "size": 1, "sha256": EXAMPLE_SHA256}]}),
            "input-file:path",
        ),
        (
            json!({"schema": "krr-macho-noso-input/v1", "files": [{"path": path, "size": -1, "sha256": EXAMPLE_SHA256}]}),
            "input-file:size",
        ),
        (
            json!({"schema": "krr-macho-noso-input/v1", "files": [{"path": path, "size": 1, "sha256": null}]}),
            "input-file:sha256",
        ),
    ] {
        assert_eq!(input_rows(&manifest).err().as_deref(), Some(expected));
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
    assert_eq!(output["path"], json!(fixture.path));
    assert_eq!(output["size"], json!(fixture.bytes.len()));
    assert_eq!(output["sha256"], fixture.digest);
    assert_eq!(
        output["nOso"],
        json!([{"symbolIndex": 0, "name": "/tmp/krr-object.o"}])
    );
    assert!(is_sha256(&fixture.digest));
    Ok(())
}

#[test]
fn inspect_file_rejects_size_or_digest_drift() -> Result<(), String> {
    let fixture = write_fixture()?;
    assert_eq!(
        inspect_file(
            fixture.path.clone(),
            (fixture.bytes.len() + SIZE_DRIFT_BYTES) as u64,
            &fixture.digest
        )
        .err()
        .as_deref(),
        Some("input-file:size-or-type-changed")
    );
    assert_eq!(
        inspect_file(
            fixture.path,
            fixture.bytes.len() as u64,
            &sha256(b"different")
        )
        .err()
        .as_deref(),
        Some("input-file:sha256-changed")
    );
    Ok(())
}

#[test]
fn inspect_file_preserves_missing_file_io_error() -> Result<(), String> {
    let (_directory, missing) = invalid_path_inputs()?;
    let error = std::fs::metadata(&missing)
        .err()
        .ok_or("missing path exists")?;
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    let actual = inspect_file(missing, 0, &sha256(b"unused"));
    assert_eq!(actual.err().as_deref(), Some(error.to_string().as_str()));
    Ok(())
}

#[test]
fn inspect_file_rejects_nonfile_and_digest_matched_invalid_binary() -> Result<(), String> {
    let fixture = write_fixture()?;
    let directory = fixture.path.parent().ok_or("fixture parent missing")?;
    assert_eq!(
        inspect_file(directory.to_path_buf(), 0, &fixture.digest)
            .err()
            .as_deref(),
        Some("input-file:size-or-type-changed")
    );
    let invalid = b"not-mach-o";
    std::fs::write(&fixture.path, invalid).map_err(|error| error.to_string())?;
    let result = inspect_file(fixture.path.clone(), invalid.len() as u64, &sha256(invalid));
    assert!(
        result
            .err()
            .is_some_and(|error| error.starts_with("macho-kind:"))
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

#[test]
fn run_rejects_invalid_manifest_schema_and_file_size() -> Result<(), String> {
    let fixture = write_fixture()?;
    let path = write_verified_manifest(&fixture)?;
    let invalid_schema = json!({"schema": "wrong", "files": []});
    std::fs::write(&path, invalid_schema.to_string()).map_err(|error| error.to_string())?;
    assert_eq!(
        MachoNoso::run(&path).err().as_deref(),
        Some("input-manifest:schema")
    );
    let wrong_size = manifest(&fixture.path, fixture.bytes.len() + 1, &fixture.digest);
    std::fs::write(&path, wrong_size.to_string()).map_err(|error| error.to_string())?;
    assert_eq!(
        MachoNoso::run(&path).err().as_deref(),
        Some("input-file:size-or-type-changed")
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn run_rejects_unreadable_manifest_and_macho_file() -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs, io::ErrorKind, os::unix::fs::PermissionsExt};

    const NO_FILE_ACCESS: u32 = 0o000;
    let fixture = write_fixture()?;
    let manifest_path = write_verified_manifest(&fixture)?;
    for path in [&manifest_path, &fixture.path] {
        let original = fs::metadata(path)?.permissions();
        fs::set_permissions(path, fs::Permissions::from_mode(NO_FILE_ACCESS))?;
        let expected = fs::read(path);
        let actual = MachoNoso::run(&manifest_path);
        fs::set_permissions(path, original)?;
        let denied = expected.err().ok_or("fixture remained readable")?;
        assert_eq!(denied.kind(), ErrorKind::PermissionDenied);
        assert_eq!(actual, Err(denied.to_string()));
    }
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
