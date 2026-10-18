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
            if is_symlink(parent)? {
                return Err(HtmlArtifactError::SymlinkPath);
            }
            current = parent.to_path_buf();
        }
        Ok(())
    }

    pub(crate) fn reject_symlink(path: &Path) -> Result<(), HtmlArtifactError> {
        if is_symlink(path)? {
            return Err(HtmlArtifactError::SymlinkPath);
        }
        Ok(())
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

fn is_symlink(path: &Path) -> Result<bool, HtmlArtifactError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.file_type().is_symlink()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
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
mod tests {
    use super::*;

    fn root(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "krr-artifact-filesystem-{label}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    #[test]
    fn write_atomic_removes_only_owned_temp_on_rename_failure()
    -> Result<(), Box<dyn std::error::Error>> {
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
}
