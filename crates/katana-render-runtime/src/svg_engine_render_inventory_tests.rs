use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub(super) const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests"
);
const PROVENANCE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/render-fixture-provenance.json"
);
const FILE_COUNT: usize = 3537;
const NORMAL_COUNT: usize = 1742;
const EXTRA_COUNT: usize = 15;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Inventory {
    source_commit: String,
    file_count: usize,
    normal_case_count: usize,
    extra_case_count: usize,
    files: Vec<FileRecord>,
    normal_cases: Vec<CaseRecord>,
    extra_cases: Vec<CaseRecord>,
}

#[derive(Deserialize)]
struct FileRecord {
    path: String,
    sha256: String,
    bytes: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaseRecord {
    path: String,
    expected_different_pixels: usize,
}

#[test]
fn render_fixture_inventory_matches_upstream_pins() -> Result<(), String> {
    let inventory = read_inventory()?;
    assert_eq!(
        inventory.source_commit,
        "c01f0165059e472f45472cccddc573d32c13548a"
    );
    assert_eq!(
        (inventory.file_count, inventory.files.len()),
        (FILE_COUNT, FILE_COUNT)
    );
    assert_eq!(
        (inventory.normal_case_count, inventory.normal_cases.len()),
        (NORMAL_COUNT, NORMAL_COUNT)
    );
    assert_eq!(
        (inventory.extra_case_count, inventory.extra_cases.len()),
        (EXTRA_COUNT, EXTRA_COUNT)
    );
    let files = verify_files(&inventory.files)?;
    verify_cases(&inventory.normal_cases, &files)?;
    verify_cases(&inventory.extra_cases, &files)?;
    super::browser_reference_tests::verify_manifest()
}

fn read_inventory() -> Result<Inventory, String> {
    let bytes = std::fs::read(PROVENANCE).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

pub(super) fn diagnostic_cases() -> Result<Vec<(PathBuf, bool)>, String> {
    let inventory = read_inventory()?;
    if inventory.normal_case_count != NORMAL_COUNT
        || inventory.normal_cases.len() != NORMAL_COUNT
        || inventory.extra_case_count != EXTRA_COUNT
        || inventory.extra_cases.len() != EXTRA_COUNT
    {
        return Err("upstream diagnostic case count mismatch".into());
    }
    let files = verify_files(&inventory.files)?;
    verify_cases(&inventory.normal_cases, &files)?;
    verify_cases(&inventory.extra_cases, &files)?;
    Ok(inventory
        .normal_cases
        .iter()
        .map(|case| (case, false))
        .chain(inventory.extra_cases.iter().map(|case| (case, true)))
        .map(|(case, extra)| {
            (
                Path::new(FIXTURE_ROOT).join(format!("{}.svg", case.path)),
                extra,
            )
        })
        .collect())
}

#[test]
fn diagnostic_paths_preserve_upstream_case_names() -> Result<(), String> {
    let inventory = read_inventory()?;
    let expected: Vec<_> = inventory
        .normal_cases
        .iter()
        .chain(&inventory.extra_cases)
        .collect();
    let actual = diagnostic_cases()?;
    assert_eq!(actual.len(), expected.len());
    let mut dotted_expected = 0;
    let mut dotted_actual = 0;
    for (case, (path, _)) in expected.iter().zip(&actual) {
        let name = Path::new(&case.path)
            .file_name()
            .ok_or("missing case name")?;
        assert!(path.is_file(), "{}", path.display());
        assert_eq!(path.file_stem(), Some(name), "{}", path.display());
        dotted_expected += usize::from(name.to_string_lossy().contains('.'));
        dotted_actual += usize::from(
            path.file_stem()
                .is_some_and(|stem| stem.to_string_lossy().contains('.')),
        );
    }
    assert!(dotted_expected > 0);
    assert_eq!(dotted_actual, dotted_expected);
    Ok(())
}

fn verify_files(records: &[FileRecord]) -> Result<BTreeSet<String>, String> {
    let mut files = BTreeSet::new();
    for record in records {
        let relative = Path::new(&record.path);
        if relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err("invalid fixture inventory path".to_owned());
        }
        if !files.insert(record.path.clone()) {
            return Err("duplicate fixture inventory path".to_owned());
        }
        let data =
            std::fs::read(Path::new(FIXTURE_ROOT).join(relative)).map_err(|e| e.to_string())?;
        assert_eq!(data.len(), record.bytes, "{}", record.path);
        assert_eq!(hex_digest(&data), record.sha256, "{}", record.path);
    }
    Ok(files)
}

pub(super) fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn verify_cases(cases: &[CaseRecord], files: &BTreeSet<String>) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for case in cases {
        if !names.insert(&case.path) || case.expected_different_pixels != 0 {
            return Err("duplicate fixture case or unexpected allowance".to_owned());
        }
        if !files.contains(&format!("{}.svg", case.path))
            || !files.contains(&format!("{}.png", case.path))
        {
            return Err(format!("missing fixture pair: {}", case.path));
        }
    }
    Ok(())
}
