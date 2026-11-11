use super::{Corruption, Fixture, MachoNosoFixture, input_rows, manifest, sha256, write_fixture};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

#[test]
fn input_rows_rejects_relative_noncanonical_and_duplicate_paths() -> Result<(), String> {
    let fixture = write_fixture()?;
    assert!(path_manifest_rejected(&duplicate_path_manifest(&fixture)));
    assert_noncanonical_path_rejected(&fixture)?;
    let relative_fixture = write_relative_fixture()?;
    let relative = manifest(
        &relative_fixture.path,
        relative_fixture.bytes.len(),
        &relative_fixture.digest,
    );
    assert!(path_manifest_rejected(&relative));
    Ok(())
}

#[test]
fn input_rows_rejects_canonical_directory_path() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory
        .path()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    assert_regular_file_required(&manifest(&path, 0, &sha256(&[])));
    Ok(())
}

fn duplicate_path_manifest(fixture: &Fixture) -> Value {
    let row = manifest(&fixture.path, fixture.bytes.len(), &fixture.digest)["files"][0].clone();
    json!({"schema": "krr-macho-noso-input/v1", "files": [row.clone(), row]})
}

fn assert_noncanonical_path_rejected(fixture: &Fixture) -> Result<(), String> {
    let path = path_with_parent_segment(&fixture.path)?;
    if !path.exists() || path.canonicalize().map_err(|error| error.to_string())? != fixture.path {
        return Err("noncanonical path must resolve to the existing fixture".to_owned());
    }
    assert_regular_file_required(&manifest(&path, fixture.bytes.len(), &fixture.digest));
    Ok(())
}

fn assert_regular_file_required(manifest: &Value) {
    assert_eq!(
        input_rows(manifest).err().as_deref(),
        Some("path is not a canonical regular file")
    );
}

fn path_manifest_rejected(manifest: &Value) -> bool {
    input_rows(manifest).is_err()
}

fn path_with_parent_segment(path: &Path) -> Result<std::path::PathBuf, String> {
    let parent = path.parent().ok_or("fixture parent")?;
    let parent_name = parent.file_name().ok_or("fixture directory name")?;
    let name = path.file_name().ok_or("fixture filename")?;
    Ok(parent.join("..").join(parent_name).join(name))
}

fn write_relative_fixture() -> Result<Fixture, String> {
    let working_directory = std::env::current_dir().map_err(|error| error.to_string())?;
    let directory = tempfile::Builder::new()
        .prefix("macho-noso-relative-")
        .tempdir_in(&working_directory)
        .map_err(|error| error.to_string())?;
    let bytes = MachoNosoFixture::thin_macho(true, Corruption::None);
    let path = directory.path().join("fixture.o");
    fs::write(&path, &bytes).map_err(|error| error.to_string())?;
    let relative_path = path
        .strip_prefix(&working_directory)
        .map_err(|error| error.to_string())?
        .to_path_buf();
    Ok(Fixture {
        _directory: directory,
        path: relative_path,
        digest: sha256(&bytes),
        bytes,
    })
}
