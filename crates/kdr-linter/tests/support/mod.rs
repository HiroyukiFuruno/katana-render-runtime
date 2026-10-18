use std::path::{Path, PathBuf};

mod files;

pub(super) use files::{TestResult, temp_root, write_file};

pub(super) fn write_manifest_pair(root: &Path, library: &str, cli: &str) -> TestResult<()> {
    write_file(root, "crates/katana-render-runtime/Cargo.toml", library)?;
    write_file(root, "crates/katana-render-runtime-cli/Cargo.toml", cli)?;
    Ok(())
}

#[cfg(unix)]
pub(super) fn set_mode(path: &Path, mode: u32) -> TestResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(mode);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(unix)]
pub(super) fn blocked_directory(root: &Path, relative: &str) -> TestResult<PathBuf> {
    let blocked = root.join(relative);
    std::fs::create_dir_all(&blocked)?;
    set_mode(&blocked, 0o000)?;
    Ok(blocked)
}
