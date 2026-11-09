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
    let directory = owned_output_directory("raw-geometry-override-")?;
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
fn filesystem_root_is_rejected_without_a_parent() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .last()
        .ok_or("workspace filesystem root missing")?;
    assert!(root.is_absolute() && root.parent().is_none());
    let output = DiagnosticOutput::from_override(Some(root.as_os_str().to_owned()))?;
    assert_eq!(
        output.write(b"diagnostic"),
        Err("diagnostic output parent is missing".to_owned())
    );
    Ok(())
}

#[test]
fn missing_parent_under_owned_tmp_is_not_created() -> Result<(), String> {
    let directory = owned_output_directory("raw-geometry-missing-parent-")?;
    let missing_parent = directory.path().join("missing");
    let expected = std::fs::canonicalize(&missing_parent)
        .err()
        .ok_or("missing output parent unexpectedly canonicalized")?;
    assert_eq!(expected.kind(), std::io::ErrorKind::NotFound);
    let output =
        DiagnosticOutput::from_override(Some(missing_parent.join("result.json").into_os_string()))?;
    assert_eq!(output.write(b"diagnostic"), Err(expected.to_string()));
    assert!(!missing_parent.exists());
    Ok(())
}

#[test]
fn existing_child_parent_path_without_filename_is_rejected() -> Result<(), String> {
    let directory = owned_output_directory("raw-geometry-parent-path-")?;
    let child = directory.path().join("child");
    std::fs::create_dir(&child).map_err(|error| error.to_string())?;
    let output = DiagnosticOutput::from_override(Some(child.join("..").into_os_string()))?;
    assert_eq!(
        output.write(b"diagnostic"),
        Err("diagnostic output filename is missing".to_owned())
    );
    assert!(
        std::fs::read_dir(child)
            .map_err(|error| error.to_string())?
            .next()
            .is_none()
    );
    Ok(())
}

fn owned_output_directory(prefix: &str) -> Result<tempfile::TempDir, String> {
    let tmp = workspace_tmp()?;
    std::fs::create_dir_all(&tmp).map_err(|error| error.to_string())?;
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(tmp)
        .map_err(|error| error.to_string())
}

#[test]
fn explicit_path_outside_workspace_tmp_is_rejected() -> Result<(), String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("diagnostic.json");
    let output = DiagnosticOutput::from_override(Some(path.into_os_string()))?;
    assert!(output.write(b"diagnostic").is_err());
    Ok(())
}
