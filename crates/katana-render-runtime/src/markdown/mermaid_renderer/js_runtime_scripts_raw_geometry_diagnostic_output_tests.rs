use super::{DiagnosticOutput, workspace_tmp};
use std::{ffi::OsString, path::Path};

#[test]
fn automatic_output_is_fresh_and_removed_on_drop() -> Result<(), String> {
    let output = DiagnosticOutput::from_override(None)?;
    let path = output.path.clone();
    output.write(b"diagnostic")?;
    assert!(path.is_file());
    assert!(output.write(b"replacement").is_err());
    drop(output);
    assert!(!path.exists());
    Ok(())
}

#[test]
fn explicit_output_is_exclusive_and_remains_on_drop() -> Result<(), String> {
    let tmp = workspace_tmp()?;
    std::fs::create_dir_all(&tmp).map_err(|error| error.to_string())?;
    let directory = tempfile::Builder::new()
        .prefix("raw-geometry-override-")
        .tempdir_in(tmp)
        .map_err(|error| error.to_string())?;
    let path = directory.path().join("result.json");
    let output = DiagnosticOutput::from_override(Some(path.clone().into_os_string()))?;
    output.write(b"diagnostic")?;
    assert!(output.write(b"replacement").is_err());
    drop(output);
    assert_eq!(
        std::fs::read(path).map_err(|error| error.to_string())?,
        b"diagnostic"
    );
    Ok(())
}

#[test]
fn invalid_explicit_relative_path_is_rejected() {
    assert!(DiagnosticOutput::from_override(Some(OsString::from("relative.json"))).is_err());
}

#[test]
fn explicit_path_outside_workspace_tmp_is_rejected() -> Result<(), String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("diagnostic.json");
    let output = DiagnosticOutput::from_override(Some(path.into_os_string()))?;
    assert!(output.write(b"diagnostic").is_err());
    Ok(())
}
