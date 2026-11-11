#![cfg(test)]

use super::{input_rows, sha256};
use crate::macho_noso_test_support::{Corruption, MachoNosoFixture};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[path = "macho_noso_contract_tests.rs"]
mod contract_tests;
#[path = "macho_noso_input_path_tests.rs"]
mod input_path_tests;

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

fn invalid_path_inputs() -> Result<(tempfile::TempDir, std::path::PathBuf), String> {
    let directory = tempdir().map_err(|error| error.to_string())?;
    let missing = directory.path().join("missing");
    Ok((directory, missing))
}

fn write_verified_manifest(fixture: &Fixture) -> Result<std::path::PathBuf, String> {
    let path = fixture.path.with_file_name("manifest.json");
    let bytes = serde_json::to_vec(&manifest(
        &fixture.path,
        fixture.bytes.len(),
        &fixture.digest,
    ))
    .map_err(|error| error.to_string())?;
    fs::write(&path, bytes).map_err(|error| error.to_string())?;
    Ok(path)
}

fn write_malformed_manifest(fixture: &Fixture) -> Result<std::path::PathBuf, String> {
    let path = fixture.path.with_file_name("malformed.json");
    fs::write(&path, b"[").map_err(|error| error.to_string())?;
    Ok(path)
}

#[test]
fn input_rows_rejects_canonical_directory_path() -> Result<(), String> {
    let directory = tempdir().map_err(|error| error.to_string())?;
    let path = directory
        .path()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let manifest = manifest(&path, 0, &sha256(&[]));

    assert_eq!(
        input_rows(&manifest).err().as_deref(),
        Some("path is not a canonical regular file")
    );
    Ok(())
}
