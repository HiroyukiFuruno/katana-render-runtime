use super::*;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::system::file_size_limit_test_support::FileSizeLimitTest;

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn lock_write_failure_releases_owned_lock_for_retry() -> Result<(), Box<dyn std::error::Error>> {
    FileSizeLimitTest::run_in_child(
        module_path!(),
        "lock_write_failure_releases_owned_lock_for_retry",
        || {
            let directory = tempfile::tempdir()?;
            let root = directory.path();
            let result = FileSizeLimitTest::with_zero_limit(|| StoreLock::acquire(root))?;
            assert_file_size_limit_error(result);
            assert!(!root.join(".krr-artifact.lock").exists());
            let retry = StoreLock::acquire(root)?;
            drop(retry);
            assert!(!root.join(".krr-artifact.lock").exists());
            Ok(())
        },
    )
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn atomic_write_failure_keeps_original_and_releases_temp_for_retry()
-> Result<(), Box<dyn std::error::Error>> {
    FileSizeLimitTest::run_in_child(
        module_path!(),
        "atomic_write_failure_keeps_original_and_releases_temp_for_retry",
        assert_atomic_write_recovers,
    )
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn assert_atomic_write_recovers() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("frame.json");
    let temp = target.with_extension(format!("tmp-{}", std::process::id()));
    let original = b"original";
    let replacement = b"replacement";
    fs::write(&target, original)?;
    let result = FileSizeLimitTest::with_zero_limit(|| {
        ArtifactFilesystem::write_atomic(&target, replacement)
    })?;
    assert_file_size_limit_error(result);
    assert_eq!(fs::read(&target)?, original);
    assert!(!temp.exists());
    ArtifactFilesystem::write_atomic(&target, replacement)?;
    assert_eq!(fs::read(&target)?, replacement);
    assert!(!temp.exists());
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn assert_file_size_limit_error<T>(result: Result<T, HtmlArtifactError>) {
    assert!(matches!(
        result,
        Err(HtmlArtifactError::Io(error))
            if error.raw_os_error() == FileSizeLimitTest::limit_error().raw_os_error()
    ));
}

fn root(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!(
        "krr-artifact-filesystem-{label}-{}",
        std::process::id()
    ));
    fs::create_dir_all(&path)?;
    Ok(path)
}

#[test]
fn write_atomic_removes_only_owned_temp_on_rename_failure() -> Result<(), Box<dyn std::error::Error>>
{
    let root = root("rename")?;
    let target = root.join("frame.json");
    fs::create_dir(&target)?;
    let result = ArtifactFilesystem::write_atomic(&target, b"frame");
    assert!(matches!(result, Err(HtmlArtifactError::Io(_))));
    assert!(
        !root
            .join(format!("frame.tmp-{}", std::process::id()))
            .exists()
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn lock_and_missing_path_errors_are_reported() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "krr-artifact-filesystem-missing-{}",
        std::process::id()
    ));
    assert!(matches!(
        StoreLock::acquire(&root),
        Err(HtmlArtifactError::Io(_))
    ));
    assert!(ArtifactFilesystem::reject_symlink(&root.join("missing")).is_ok());
    assert!(ArtifactFilesystem::reject_ancestor_symlink(Path::new("/")).is_ok());
    assert!(matches!(
        ArtifactFilesystem::reject_symlink(Path::new("\0")),
        Err(HtmlArtifactError::Io(_))
    ));
    Ok(())
}

#[test]
fn store_lock_lifecycle_rejects_competing_owner_and_releases_on_drop()
-> Result<(), Box<dyn std::error::Error>> {
    let root = root("lock-lifecycle")?;
    let first = StoreLock::acquire(&root)?;
    assert_eq!(
        fs::read_to_string(root.join(".krr-artifact.lock"))?,
        std::process::id().to_string()
    );
    assert!(matches!(
        StoreLock::acquire(&root),
        Err(HtmlArtifactError::StoreBusy)
    ));
    drop(first);
    assert!(!root.join(".krr-artifact.lock").exists());
    let second = StoreLock::acquire(&root)?;
    drop(second);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn write_atomic_commits_bytes_and_removes_temp_name() -> Result<(), Box<dyn std::error::Error>> {
    let root = root("write-success")?;
    let target = root.join("frame.json");
    ArtifactFilesystem::write_atomic(&target, b"frame")?;
    assert_eq!(fs::read(&target)?, b"frame");
    assert!(
        !root
            .join(format!("frame.tmp-{}", std::process::id()))
            .exists()
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn store_bytes_and_ancestor_reject_symlinks() -> Result<(), Box<dyn std::error::Error>> {
    let root = root("symlink")?;
    fs::write(root.join("frame.json"), b"frame")?;
    fs::write(root.join(".krr-artifact.lock"), b"lock")?;
    std::os::unix::fs::symlink("missing", root.join("linked"))?;
    assert!(matches!(
        ArtifactFilesystem::store_bytes(&root),
        Err(HtmlArtifactError::SymlinkPath)
    ));
    let parent = root.join("parent");
    fs::create_dir_all(&parent)?;
    std::os::unix::fs::symlink(&parent, root.join("alias"))?;
    assert!(matches!(
        ArtifactFilesystem::reject_ancestor_symlink(&root.join("alias/frame")),
        Err(HtmlArtifactError::SymlinkPath)
    ));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn store_bytes_rejects_depth_and_entry_limits_and_ignores_other_file_types()
-> Result<(), Box<dyn std::error::Error>> {
    assert_depth_limit()?;
    assert_entry_limit()?;
    assert_special_file_is_ignored()?;
    Ok(())
}

fn assert_depth_limit() -> Result<(), Box<dyn std::error::Error>> {
    let depth_root = root("depth-limit")?;
    let mut nested = depth_root.clone();
    for index in 0..=MAX_STORE_DEPTH {
        nested = nested.join(format!("d{index}"));
        fs::create_dir(&nested)?;
    }
    assert!(matches!(
        ArtifactFilesystem::store_bytes(&depth_root),
        Err(HtmlArtifactError::StoreTraversalLimit)
    ));
    fs::remove_dir_all(depth_root)?;
    Ok(())
}

fn assert_entry_limit() -> Result<(), Box<dyn std::error::Error>> {
    let entries_root = root("entry-limit")?;
    fs::write(entries_root.join("frame"), b"frame")?;
    let mut state = TraversalState {
        entries: MAX_STORE_ENTRIES,
    };
    assert!(matches!(
        sum_regular_files(&entries_root, 0, &mut state),
        Err(HtmlArtifactError::StoreTraversalLimit)
    ));
    fs::remove_dir_all(entries_root)?;
    Ok(())
}

#[cfg(unix)]
fn assert_special_file_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
    let special_root = root("special-file")?;
    let fifo = special_root.join("frame.fifo");
    nix::unistd::mkfifo(
        &fifo,
        nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
    )?;
    assert_eq!(ArtifactFilesystem::store_bytes(&special_root)?, 0);
    fs::remove_file(fifo)?;
    fs::remove_dir_all(special_root)?;
    Ok(())
}

#[cfg(not(unix))]
fn assert_special_file_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[test]
fn atomic_write_preserves_existing_temp_and_destination() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("frame.json");
    let temp = target.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&target, b"old frame")?;
    fs::write(&temp, b"other writer")?;

    assert!(matches!(
        ArtifactFilesystem::write_atomic(&target, b"new frame"),
        Err(HtmlArtifactError::Io(error)) if error.kind() == std::io::ErrorKind::AlreadyExists
    ));
    assert_eq!(fs::read(&target)?, b"old frame");
    assert_eq!(fs::read(&temp)?, b"other writer");
    Ok(())
}

#[test]
fn quota_ignores_only_root_lock_and_counts_nested_lock() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let nested = directory.path().join("nested");
    fs::create_dir(&nested)?;
    fs::write(directory.path().join(".krr-artifact.lock"), b"root lock")?;
    fs::write(nested.join(".krr-artifact.lock"), b"nested")?;
    fs::write(nested.join("frame"), b"frame")?;

    assert_eq!(ArtifactFilesystem::store_bytes(directory.path())?, 11);
    Ok(())
}

#[cfg(unix)]
#[test]
fn lock_rejects_symlink_without_changing_target() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("target");
    let lock = directory.path().join(".krr-artifact.lock");
    fs::write(&target, b"keep")?;
    std::os::unix::fs::symlink(&target, &lock)?;

    assert!(matches!(
        StoreLock::acquire(directory.path()),
        Err(HtmlArtifactError::SymlinkPath)
    ));
    assert_eq!(fs::read(&target)?, b"keep");
    assert!(fs::symlink_metadata(&lock)?.file_type().is_symlink());
    Ok(())
}
