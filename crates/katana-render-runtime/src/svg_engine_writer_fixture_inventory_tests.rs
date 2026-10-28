/* Copyright 2023 the Resvg Authors
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;
use sha2::{Digest, Sha256};

const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream"
);
const FONT_DIRECTORY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts"
);
const VENDORED_PROVENANCE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/vendor/svg-engine/PROVENANCE.json"
);

struct Inventory {
    count: usize,
    bytes: u64,
    sha256: String,
}

#[test]
fn writer_fixtures_match_recorded_source_inventory_and_licenses() -> Result<(), String> {
    let provenance = read_json(&Path::new(FIXTURE_ROOT).join("PROVENANCE.json"))?;
    let vendor = read_json(Path::new(VENDORED_PROVENANCE))?;
    assert_eq!(
        required_str(&provenance, "sourceCommit")?,
        required_str(&vendor, "sourceCommit")?
    );
    verify_writer_cases(&provenance)?;
    verify_inventory(
        &provenance,
        "writerFixtures",
        &Path::new(FIXTURE_ROOT).join("usvg/tests/files"),
    )?;
    verify_inventory(&provenance, "fontFiles", Path::new(FONT_DIRECTORY))?;
    verify_licenses(&provenance, &vendor)
}

fn verify_writer_cases(provenance: &Value) -> Result<(), String> {
    let records = case_records(provenance)?;
    assert_eq!(records.len(), 31, "writer fixture case count");
    assert_eq!(mode_count(&records, "default"), 21);
    assert_eq!(mode_count(&records, "preserve-text"), 9);
    assert_eq!(mode_count(&records, "id-prefix"), 1);
    let actual = fixture_case_names(&Path::new(FIXTURE_ROOT).join("usvg/tests/files"))?;
    assert_eq!(
        actual,
        records.keys().cloned().collect(),
        "writer cases match provenance"
    );
    Ok(())
}

fn case_records(provenance: &Value) -> Result<BTreeMap<String, String>, String> {
    let cases = provenance["cases"]
        .as_array()
        .ok_or("provenance cases must be an array")?;
    let mut records = BTreeMap::new();
    for case in cases {
        let name = required_str(case, "name")?;
        let mode = required_str(case, "mode")?;
        validate_case_mode(case, name, mode)?;
        if records.insert(name.to_owned(), mode.to_owned()).is_some() {
            return Err(format!("duplicate writer case {name}"));
        }
    }
    Ok(records)
}

fn validate_case_mode(case: &Value, name: &str, mode: &str) -> Result<(), String> {
    if !["default", "preserve-text", "id-prefix"].contains(&mode) {
        return Err(format!("unsupported writer mode {mode} for {name}"));
    }
    if mode == "id-prefix" && case["value"].as_str() != Some("prefix-") {
        return Err("id-prefix case must use the recorded prefix- value".to_owned());
    }
    Ok(())
}

fn mode_count(records: &BTreeMap<String, String>, mode: &str) -> usize {
    records
        .values()
        .filter(|value| value.as_str() == mode)
        .count()
}

fn fixture_case_names(directory: &Path) -> Result<BTreeSet<String>, String> {
    let files = inventory_files(directory)?;
    assert_eq!(files.len(), 62, "writer input and expected file count");
    let mut names = BTreeSet::new();
    for path in files {
        if let Some(name) = input_case_name(&path)? {
            if !path
                .with_file_name(format!("{name}-expected.svg"))
                .is_file()
            {
                return Err(format!("writer expectation missing for {name}"));
            }
            names.insert(name);
        }
    }
    Ok(names)
}

fn input_case_name(path: &Path) -> Result<Option<String>, String> {
    let file = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("writer filename is not UTF-8")?;
    if !file.ends_with(".svg") || file.ends_with("-expected.svg") {
        return Ok(None);
    }
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(|name| Some(name.to_owned()))
        .ok_or_else(|| format!("writer case name is not UTF-8: {}", path.display()))
}

fn verify_inventory(provenance: &Value, field: &str, directory: &Path) -> Result<(), String> {
    let actual = inventory(directory)?;
    let record = &provenance[field];
    let count = usize::try_from(
        record["fileCount"]
            .as_u64()
            .ok_or("inventory fileCount missing")?,
    )
    .map_err(error_string)?;
    assert_eq!(actual.count, count, "{field} file count");
    assert_eq!(
        actual.bytes,
        record["byteCount"]
            .as_u64()
            .ok_or("inventory byteCount missing")?,
        "{field} byte count"
    );
    assert_eq!(
        actual.sha256,
        required_str(record, "inventorySha256")?,
        "{field} SHA-256"
    );
    Ok(())
}

fn verify_licenses(provenance: &Value, vendor: &Value) -> Result<(), String> {
    let manifest = provenance["licenseFiles"]
        .as_array()
        .ok_or("fixture licenseFiles must be an array")?;
    let vendor_licenses = vendor["licenses"]
        .as_array()
        .ok_or("vendor licenses must be an array")?;
    assert_eq!(manifest.len(), 2, "recorded license file count");
    for entry in manifest {
        verify_license_entry(entry, vendor_licenses)?;
    }
    Ok(())
}

fn verify_license_entry(entry: &Value, vendor_licenses: &[Value]) -> Result<(), String> {
    let relative = required_str(entry, "path")?;
    let actual = digest_file(&Path::new(FIXTURE_ROOT).join(relative))?;
    assert_eq!(
        actual,
        required_str(entry, "sha256")?,
        "fixture license {relative}"
    );
    let vendor = vendor_licenses
        .iter()
        .find(|item| item["destination"] == relative)
        .ok_or_else(|| format!("vendored provenance lacks license {relative}"))?;
    assert_eq!(
        actual,
        required_str(vendor, "transplantedSha256")?,
        "vendored license {relative}"
    );
    assert_eq!(
        actual,
        required_str(vendor, "sourceSha256")?,
        "upstream license {relative}"
    );
    Ok(())
}

fn inventory(directory: &Path) -> Result<Inventory, String> {
    let files = inventory_files(directory)?;
    let count = files.len();
    let mut bytes = 0_u64;
    let mut rows = String::new();
    for path in &files {
        let relative = path
            .strip_prefix(directory)
            .map_err(error_string)?
            .to_string_lossy()
            .replace('\\', "/");
        let content = fs::read(path).map_err(error_string)?;
        bytes = bytes
            .checked_add(u64::try_from(content.len()).map_err(error_string)?)
            .ok_or("fixture inventory byte count overflow")?;
        rows.push_str(&format!(
            "{relative}\t{}\t{}\n",
            content.len(),
            hex_digest(&content)
        ));
    }
    Ok(Inventory {
        count,
        bytes,
        sha256: hex_digest(rows.as_bytes()),
    })
}

fn inventory_files(directory: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let mut files = BTreeSet::new();
    collect_files(directory, &mut files)?;
    Ok(files)
}

fn collect_files(directory: &Path, files: &mut BTreeSet<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(directory).map_err(error_string)? {
        let entry = entry.map_err(error_string)?;
        let path = entry.path();
        let kind = entry.file_type().map_err(error_string)?;
        if kind.is_dir() {
            collect_files(&path, files)?;
        } else if kind.is_file() {
            files.insert(path);
        } else {
            return Err(format!(
                "unsupported fixture entry type: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&fs::read(path).map_err(error_string)?).map_err(error_string)
}

fn digest_file(path: &Path) -> Result<String, String> {
    Ok(hex_digest(&fs::read(path).map_err(error_string)?))
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value[field]
        .as_str()
        .ok_or_else(|| format!("provenance field {field} must be a string"))
}

fn error_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}
