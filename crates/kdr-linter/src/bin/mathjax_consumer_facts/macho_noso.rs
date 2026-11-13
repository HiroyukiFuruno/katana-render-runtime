use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST_SCHEMA: &str = "krr-macho-noso-input/v1";
const REPORT_SCHEMA: &str = "kdr-linter/macho-noso-facts/v1";
const SHA256_HEX_LENGTH: usize = 64;

pub(super) struct MachoNoso;

struct InputFile {
    path: PathBuf,
    size: u64,
    sha256: String,
}

impl MachoNoso {
    pub(super) fn run(manifest_path: &Path) -> Result<(), String> {
        let manifest_path = canonical_file(manifest_path)?;
        let manifest_bytes = fs::read(&manifest_path).map_err(|error| error.to_string())?;
        let manifest_sha256 = sha256(&manifest_bytes);
        let manifest: Value = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| format!("invalid-input-manifest:{error}"))?;
        let rows = input_rows(&manifest)?;
        let parser = super::macho_noso_parser_pin::ParserPin::read()?;
        let mut files = Vec::with_capacity(rows.len());
        for row in rows {
            files.push(inspect_file(row.path, row.size, &row.sha256)?);
        }
        let output = json!({
            "schema": REPORT_SCHEMA,
            "status": "verified",
            "parser": parser,
            "manifest": {"path": manifest_path, "sha256": manifest_sha256},
            "files": files,
        })
        .to_string();
        println!("{output}");
        Ok(())
    }

    pub(super) fn records(bytes: &[u8]) -> Result<(&'static str, Vec<Value>), String> {
        super::macho_noso_binary::MachOReader::records(bytes)
    }
}

fn input_rows(manifest: &Value) -> Result<Vec<InputFile>, String> {
    exact_keys(manifest, &["schema", "files"], "input-manifest")?;
    if manifest["schema"] != MANIFEST_SCHEMA {
        return Err("input-manifest:schema".into());
    }
    let rows = manifest["files"].as_array().ok_or("input-manifest:files")?;
    if rows.is_empty() {
        return Err("input-manifest:empty-files".into());
    }
    let mut result = Vec::with_capacity(rows.len());
    let mut unique = BTreeSet::new();
    for row in rows {
        exact_keys(row, &["path", "size", "sha256"], "input-file")?;
        let path = input_path(row["path"].as_str().ok_or("input-file:path")?, &mut unique)?;
        let size = row["size"].as_u64().ok_or("input-file:size")?;
        let digest = row["sha256"].as_str().ok_or("input-file:sha256")?;
        if !is_sha256(digest) {
            return Err("input-file:sha256-format".into());
        }
        result.push(InputFile {
            path,
            size,
            sha256: digest.to_string(),
        });
    }
    Ok(result)
}

fn input_path(value: &str, unique: &mut BTreeSet<PathBuf>) -> Result<PathBuf, String> {
    const INVALID_PATH: &str = "input-file:path-not-canonical-or-duplicate";
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(INVALID_PATH.into());
    }
    canonical_file(&path)?;
    if !unique.insert(path.clone()) {
        return Err(INVALID_PATH.into());
    }
    Ok(path)
}

fn inspect_file(path: PathBuf, expected_size: u64, expected_sha256: &str) -> Result<Value, String> {
    let before = fs::metadata(&path).map_err(|error| error.to_string())?;
    if !before.is_file() || before.len() != expected_size {
        return Err("input-file:size-or-type-changed".into());
    }
    let bytes = fs::read(&path).map_err(|error| error.to_string())?;
    validate_read_at_path(&path, &before, bytes.len(), expected_size)?;
    let digest = sha256(&bytes);
    if digest != expected_sha256 {
        return Err("input-file:sha256-changed".into());
    }
    let (format, records) = MachoNoso::records(&bytes)?;
    canonical_file(&path)?;
    Ok(
        json!({"path": path, "size": expected_size, "sha256": digest,
              "format": format, "nOso": records}),
    )
}

fn validate_read_at_path(
    path: &Path,
    before: &fs::Metadata,
    bytes_read: usize,
    expected_size: u64,
) -> Result<(), String> {
    let after = fs::metadata(path).map_err(|error| error.to_string())?;
    validate_read(before, &after, bytes_read, expected_size)
}

fn validate_read(
    before: &fs::Metadata,
    after: &fs::Metadata,
    bytes_read: usize,
    expected_size: u64,
) -> Result<(), String> {
    if before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
        || u64::try_from(bytes_read).ok() != Some(expected_size)
    {
        return Err("input-file:changed-during-read".into());
    }
    Ok(())
}

fn exact_keys(value: &Value, expected: &[&str], label: &str) -> Result<(), String> {
    let object = value.as_object().ok_or_else(|| format!("{label}:object"))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(format!("{label}:keys"));
    }
    Ok(())
}

fn canonical_file(path: &Path) -> Result<PathBuf, String> {
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    if canonical != path
        || !fs::metadata(path)
            .map_err(|error| error.to_string())?
            .is_file()
    {
        return Err("path is not a canonical regular file".into());
    }
    Ok(canonical)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_sha256(value: &str) -> bool {
    value.len() == SHA256_HEX_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
#[path = "macho_noso_tests_private.rs"]
mod private_tests;
