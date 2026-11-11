use super::{assert_exit_one, run_cli, sha256};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::{fs, path::Path};

#[path = "../../src/bin/mathjax_consumer_facts/macho_noso_test_support.rs"]
mod fixture;

use fixture::{Corruption, MachoNosoFixture};

#[test]
fn verified_macho_inputs_produce_a_complete_report() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let folder = directory
        .path()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let inputs = [
        write_thin_input(&folder.join("fixture32.o"), false)?,
        write_thin_input(&folder.join("fixture64.o"), true)?,
    ];
    let manifest = write_manifest(&folder, &inputs)?;
    let result = run_cli(&manifest_arguments(&manifest))?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value =
        serde_json::from_slice(&result.stdout).map_err(|error| error.to_string())?;
    assert_eq!(report["schema"], "kdr-linter/macho-noso-facts/v1");
    assert_eq!(report["status"], "verified");
    let rows = report["files"].as_array().ok_or("missing verified files")?;
    assert_eq!(rows.len(), inputs.len());
    for ((row, input), format) in rows.iter().zip(inputs).zip(["MachO32", "MachO64"]) {
        assert_verified_input(row, &input, format);
    }
    Ok(())
}

#[test]
fn invalid_macho_never_emits_a_partial_success_report() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let folder = directory
        .path()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let valid = write_thin_input(&folder.join("valid.o"), true)?;
    let invalid = [
        MachoNosoFixture::thin_macho(true, Corruption::LoadCommandSize),
        MachoNosoFixture::thin_macho(true, Corruption::SymbolOffset),
        MachoNosoFixture::thin_macho(true, Corruption::NameOffset),
        MachoNosoFixture::thin_macho(true, Corruption::InvalidUtf8),
        MachoNosoFixture::fat_macho(),
    ];
    for bytes in invalid {
        let rejected = write_input(&folder.join("rejected.o"), &bytes)?;
        let manifest = write_manifest(&folder, &[valid.clone(), rejected])?;
        let result = run_cli(&manifest_arguments(&manifest))?;
        assert_exit_one(&result, "corrupted Mach-O invocation")?;
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
    }
    Ok(())
}

fn write_thin_input(path: &Path, is_64: bool) -> Result<Value, String> {
    write_input(path, &MachoNosoFixture::thin_macho(is_64, Corruption::None))
}

fn write_input(path: &Path, bytes: &[u8]) -> Result<Value, String> {
    fs::write(path, bytes).map_err(|error| error.to_string())?;
    let digest = sha256(bytes);
    Ok(json!({"path": path, "size": bytes.len(), "sha256": digest}))
}

fn write_manifest(folder: &Path, files: &[Value]) -> Result<std::path::PathBuf, String> {
    let path = folder.join("input.json");
    let data = json!({"schema": "krr-macho-noso-input/v1", "files": files});
    fs::write(&path, data.to_string()).map_err(|error| error.to_string())?;
    Ok(path)
}

fn manifest_arguments(path: &Path) -> [OsString; 2] {
    [OsString::from("--macho-noso"), path.as_os_str().to_owned()]
}

fn assert_verified_input(actual: &Value, input: &Value, format: &str) {
    for field in ["path", "size", "sha256"] {
        assert_eq!(actual[field], input[field], "{field}");
    }
    assert_eq!(actual["format"], format);
    assert_eq!(
        actual["nOso"],
        json!([{"symbolIndex": 0, "name": "/tmp/krr-object.o"}])
    );
}
