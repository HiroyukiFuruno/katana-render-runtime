use super::types::HtmlArtifactError;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub(crate) struct ArtifactFilesystem;
pub(crate) struct StoreLock {
    path: PathBuf,
}

impl StoreLock {
    pub(crate) fn acquire(root: &Path) -> Result<Self, HtmlArtifactError> {
        let path = root.join(".krr-artifact.lock");
        ArtifactFilesystem::reject_symlink(&path)?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = match options.open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(HtmlArtifactError::StoreBusy);
            }
            Err(error) => return Err(error.into()),
        };
        let guard = Self { path };
        set_private(&file)?;
        file.write_all(std::process::id().to_string().as_bytes())?;
        file.sync_all()?;
        Ok(guard)
    }
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl ArtifactFilesystem {
    pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), HtmlArtifactError> {
        let temp = path.with_extension(format!("tmp-{}", std::process::id()));
        let mut owned_temp = false;
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            owned_temp = true;
            set_private(&file)?;
            file.write_all(bytes)?;
            file.flush()?;
            file.sync_all()?;
            fs::rename(&temp, path)?;
            Ok::<(), std::io::Error>(())
        })();
        if result.is_err() && owned_temp {
            let _ = fs::remove_file(&temp);
        }
        result.map_err(Into::into)
    }

    pub(crate) fn store_bytes(root: &Path) -> Result<u64, HtmlArtifactError> {
        let mut state = TraversalState::default();
        sum_regular_files(root, 0, &mut state)
    }

    pub(crate) fn reject_ancestor_symlink(path: &Path) -> Result<(), HtmlArtifactError> {
        let mut current = path.to_path_buf();
        while let Some(parent) = current.parent() {
            Self::reject_symlink(parent)?;
            current = parent.to_path_buf();
        }
        Ok(())
    }

    pub(crate) fn reject_symlink(path: &Path) -> Result<(), HtmlArtifactError> {
        Self::metadata_without_symlink(path).map(|_| ())
    }

    pub(crate) fn metadata_without_symlink(
        path: &Path,
    ) -> Result<Option<fs::Metadata>, HtmlArtifactError> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(HtmlArtifactError::SymlinkPath)
            }
            Ok(metadata) => Ok(Some(metadata)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

const MAX_STORE_DEPTH: usize = 64;
const MAX_STORE_ENTRIES: u64 = 1_000_000;

#[derive(Default)]
struct TraversalState {
    entries: u64,
}

fn sum_regular_files(
    directory: &Path,
    depth: usize,
    state: &mut TraversalState,
) -> Result<u64, HtmlArtifactError> {
    if depth > MAX_STORE_DEPTH {
        return Err(HtmlArtifactError::StoreTraversalLimit);
    }
    let mut total = 0u64;
    for entry in fs::read_dir(directory)? {
        state.entries = state.entries.saturating_add(1);
        if state.entries > MAX_STORE_ENTRIES {
            return Err(HtmlArtifactError::StoreTraversalLimit);
        }
        total = total.saturating_add(sum_entry(entry?.path(), depth, state)?);
    }
    Ok(total)
}

fn sum_entry(
    path: PathBuf,
    depth: usize,
    state: &mut TraversalState,
) -> Result<u64, HtmlArtifactError> {
    if depth == 0
        && path
            .file_name()
            .is_some_and(|name| name == ".krr-artifact.lock")
    {
        return Ok(0);
    }
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() {
        return Err(HtmlArtifactError::SymlinkPath);
    }
    if metadata.is_dir() {
        return sum_regular_files(&path, depth + 1, state);
    }
    if metadata.is_file() {
        Ok(metadata.len())
    } else {
        Ok(0)
    }
}

#[cfg(unix)]
fn set_private(file: &fs::File) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))
}
#[cfg(not(unix))]
fn set_private(_file: &fs::File) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(unix)]
const PRIVATE_FILE_MODE: u32 = 0o600;

#[cfg(test)]
#[path = "filesystem_tests.rs"]
mod tests;
