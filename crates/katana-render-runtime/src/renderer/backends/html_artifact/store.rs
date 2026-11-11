use super::artifact::HtmlFrameArtifact;
use super::filesystem::{ArtifactFilesystem, StoreLock};
use super::key::validate_key;
use super::types::{HtmlArtifactError, HtmlArtifactProvenance, MAX_ARTIFACT_BYTES};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const LOCK_NAME: &str = ".krr-artifact.lock";
const FILE_SUFFIX: &str = ".krr-frame.json";

pub struct HtmlArtifactStore {
    root: PathBuf,
    capacity: u64,
    enabled: bool,
}

impl HtmlArtifactStore {
    /// Creates a host-owned store; callers must provide a private root and exclusive writer use.
    pub fn new(
        root: impl AsRef<Path>,
        capacity: u64,
        enabled: bool,
    ) -> Result<Self, HtmlArtifactError> {
        let root = root.as_ref();
        if !enabled {
            return Ok(Self {
                root: root.to_path_buf(),
                capacity,
                enabled,
            });
        }
        ArtifactFilesystem::reject_symlink(root)?;
        ArtifactFilesystem::reject_ancestor_symlink(root)?;
        fs::create_dir_all(root)?;
        ArtifactFilesystem::reject_symlink(root)?;
        Ok(Self {
            root: root.canonicalize()?,
            capacity,
            enabled,
        })
    }

    pub fn save(&self, key: &str, artifact: &HtmlFrameArtifact) -> Result<(), HtmlArtifactError> {
        self.require_enabled()?;
        validate_key(key)?;
        artifact.provenance.validate_current_runtime()?;
        let limit = self.capacity.min(MAX_ARTIFACT_BYTES);
        let bytes = artifact
            .encode_with_limit(limit)
            .map_err(|error| map_save_error(error, self.capacity))?;
        let _lock = StoreLock::acquire(&self.root)?;
        let path = self.path_for(key)?;
        let existing = match fs::metadata(&path) {
            Ok(meta) => meta.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error.into()),
        };
        let total = ArtifactFilesystem::store_bytes(&self.root)?
            .saturating_sub(existing)
            .saturating_add(bytes.len() as u64);
        if total > self.capacity {
            return Err(HtmlArtifactError::CapacityExceeded);
        }
        ArtifactFilesystem::write_atomic(&path, &bytes)
    }

    pub fn load(
        &self,
        key: &str,
        expected: &HtmlArtifactProvenance,
    ) -> Result<Option<HtmlFrameArtifact>, HtmlArtifactError> {
        self.require_enabled()?;
        expected.validate_current_runtime()?;
        let path = self.path_for(key)?;
        let file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let bytes = read_bounded_artifact(file, self.capacity)?;
        let artifact =
            HtmlFrameArtifact::decode_with_limit(&bytes, self.capacity.min(MAX_ARTIFACT_BYTES))?;
        artifact.validate()?;
        if artifact.provenance != *expected {
            return Ok(None);
        }
        Ok(Some(artifact))
    }

    pub fn clear(&self, key: &str) -> Result<bool, HtmlArtifactError> {
        self.require_enabled()?;
        let _lock = StoreLock::acquire(&self.root)?;
        let path = self.path_for(key)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// Removes a lock after the host proves every writer for this root has stopped.
    ///
    /// The store cannot verify writer liveness; removing a live lock permits concurrent writes.
    pub fn recover_lock(&self) -> Result<bool, HtmlArtifactError> {
        /* WHY: 稼働中ロックの削除は同時書込みとquota計算の破綻を招くため、host確認を要求する。 */
        self.require_enabled()?;
        let path = self.root.join(LOCK_NAME);
        ArtifactFilesystem::reject_symlink(&path)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn require_enabled(&self) -> Result<(), HtmlArtifactError> {
        if self.enabled {
            Ok(())
        } else {
            Err(HtmlArtifactError::Disabled)
        }
    }
    fn path_for(&self, key: &str) -> Result<PathBuf, HtmlArtifactError> {
        validate_key(key)?;
        let path = self.root.join(format!("{key}{FILE_SUFFIX}"));
        ArtifactFilesystem::reject_symlink(&path)?;
        Ok(path)
    }
}

fn read_bounded_artifact(file: fs::File, capacity: u64) -> Result<Vec<u8>, HtmlArtifactError> {
    let size = file.metadata()?.len();
    let limit = capacity.min(MAX_ARTIFACT_BYTES);
    if size > limit {
        return Err(size_limit_error(capacity));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(size_limit_error(capacity));
    }
    Ok(bytes)
}

fn size_limit_error(capacity: u64) -> HtmlArtifactError {
    if capacity < MAX_ARTIFACT_BYTES {
        HtmlArtifactError::CapacityExceeded
    } else {
        HtmlArtifactError::ArtifactTooLarge
    }
}

fn map_save_error(error: HtmlArtifactError, capacity: u64) -> HtmlArtifactError {
    match error {
        HtmlArtifactError::ArtifactTooLarge if capacity < MAX_ARTIFACT_BYTES => {
            HtmlArtifactError::CapacityExceeded
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_constructor_creates_missing_directories_and_preserves_a_conflicting_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let parent = directory.path().canonicalize()?;
        let root = parent.join("missing").join("frames");
        assert!(!root.exists());
        let store = HtmlArtifactStore::new(&root, MAX_ARTIFACT_BYTES, true)?;
        assert_eq!(store.root, root.canonicalize()?);
        assert!(root.is_dir());

        let occupied = parent.join("occupied");
        let bytes = b"existing contents";
        fs::write(&occupied, bytes)?;
        assert!(matches!(
            HtmlArtifactStore::new(&occupied, MAX_ARTIFACT_BYTES, true),
            Err(HtmlArtifactError::Io(_))
        ));
        assert_eq!(fs::read(&occupied)?, bytes);
        Ok(())
    }

    #[test]
    fn save_error_mapping_preserves_non_size_errors() {
        let error = HtmlArtifactError::InvalidKey;
        assert!(matches!(
            map_save_error(error, MAX_ARTIFACT_BYTES),
            HtmlArtifactError::InvalidKey
        ));
    }

    #[test]
    fn size_limit_error_reports_store_and_format_limits() {
        assert!(matches!(
            size_limit_error(MAX_ARTIFACT_BYTES - 1),
            HtmlArtifactError::CapacityExceeded
        ));
        assert!(matches!(
            size_limit_error(MAX_ARTIFACT_BYTES),
            HtmlArtifactError::ArtifactTooLarge
        ));
    }

    #[cfg(unix)]
    #[test]
    fn read_bounded_artifact_rechecks_stream_bytes_after_zero_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::{io::Write, thread};

        let root = std::env::temp_dir().join(format!("krr-artifact-fifo-{}", std::process::id()));
        fs::create_dir_all(&root)?;
        let fifo = root.join("frame");
        nix::unistd::mkfifo(
            &fifo,
            nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
        )?;
        let writer_path = fifo.clone();
        let writer = thread::spawn(move || -> Result<(), std::io::Error> {
            let mut file = fs::OpenOptions::new().write(true).open(writer_path)?;
            file.write_all(b"123456789")
        });
        let reader = fs::File::open(&fifo)?;
        let result = read_bounded_artifact(reader, 8);
        writer.join().map_err(|_| "FIFO writer panicked")??;
        fs::remove_file(&fifo)?;
        fs::remove_dir(&root)?;
        assert!(matches!(result, Err(HtmlArtifactError::CapacityExceeded)));
        Ok(())
    }
}

#[cfg(test)]
#[path = "store_real_tests.rs"]
mod real_tests;
