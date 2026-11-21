use super::{
    cli_with_missing_required_provenance_env, cli_with_provenance_paths, create_temp_dir,
    valid_inputs, TempDir,
};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

fn assert_provenance_path_rejected(
    temp: &TempDir,
    lock_path: &Path,
    source_snapshot_path: &Path,
    failed_path: &Path,
    expected_kind: ErrorKind,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected_error = fs::read(failed_path)
        .err()
        .ok_or("expected provenance input read to fail")?;
    assert_eq!(expected_error.kind(), expected_kind);
    let inputs = valid_inputs(temp)?;
    let output = temp.path().join("must-not-exist.json");
    let result = cli_with_provenance_paths(&inputs, &output, lock_path, source_snapshot_path)?;
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains(&format!("{expected_error:?}")), "{stderr}");
    assert!(!stderr.contains("worker 0 failed"), "{stderr}");
    assert!(!output.exists());
    Ok(())
}

fn real_lock_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Cargo.lock")
        .canonicalize()?)
}

#[test]
fn cli_reports_each_missing_required_provenance_environment_value(
) -> Result<(), Box<dyn std::error::Error>> {
    for missing_env in ["KRR_PERF_LOCK_PATH", "KRR_PERF_SOURCE_SNAPSHOT_PATH"] {
        let temp = create_temp_dir()?;
        let inputs = valid_inputs(&temp)?;
        let output = temp.path().join("must-not-exist.json");
        let result = cli_with_missing_required_provenance_env(&inputs, &output, missing_env)?;
        assert!(!result.status.success());
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains(&format!("{missing_env} is required")),
            "{stderr}"
        );
        assert!(!stderr.contains("worker 0 failed"), "{stderr}");
        assert!(!output.exists());
    }
    Ok(())
}

#[test]
fn cli_rejects_missing_provenance_lock_after_successful_workers(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = create_temp_dir()?;
    let missing_lock = temp.path().join("missing-Cargo.lock");
    let inputs = valid_inputs(&temp)?;
    assert_provenance_path_rejected(
        &temp,
        &missing_lock,
        &inputs.snapshot,
        &missing_lock,
        ErrorKind::NotFound,
    )
}

#[test]
fn cli_rejects_missing_provenance_source_snapshot_after_successful_workers(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = create_temp_dir()?;
    let missing_snapshot = temp.path().join("missing-source-snapshot.json");
    assert_provenance_path_rejected(
        &temp,
        &real_lock_path()?,
        &missing_snapshot,
        &missing_snapshot,
        ErrorKind::NotFound,
    )
}

#[cfg(unix)]
#[test]
fn cli_rejects_directory_as_provenance_source_snapshot_after_successful_workers(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = create_temp_dir()?;
    let snapshot_directory = temp.path().join("source-snapshot-directory");
    fs::create_dir(&snapshot_directory)?;
    assert_provenance_path_rejected(
        &temp,
        &real_lock_path()?,
        &snapshot_directory,
        &snapshot_directory,
        ErrorKind::IsADirectory,
    )
}
