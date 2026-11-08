use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub(crate) struct TempProbeDirectory {
    root: PathBuf,
    _owner: tempfile::TempDir,
}

pub(in crate::markdown::svg_rasterize) fn recursive_sfnt_path() -> Result<PathBuf, String> {
    let path = std::env::var_os("KRR_RECURSIVE_PROBE_SFNT_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/test-fixtures/font-probes/recursive-latin-crsv-normal.ttf")
        });
    if !path.is_absolute() {
        return Err("public SFNT path must be absolute".to_owned());
    }
    Ok(path)
}

impl TempProbeDirectory {
    pub(crate) fn create() -> Result<Self, Box<dyn std::error::Error>> {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest
            .parent()
            .and_then(|path| path.parent())
            .ok_or("workspace root missing")?;
        let temp_root = root.join("tmp/html-fidelity");
        std::fs::create_dir_all(&temp_root)?;
        let owner = tempfile::Builder::new()
            .prefix("text-probe-test-")
            .tempdir_in(temp_root)?;
        Ok(Self {
            root: owner.path().to_path_buf(),
            _owner: owner,
        })
    }

    pub(crate) fn file(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

pub(super) fn write_fresh(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .and_then(Path::parent)
        .ok_or("workspace root missing")?;
    let parent = path
        .parent()
        .ok_or("output parent missing")?
        .canonicalize()?;
    if !path.is_absolute() || !parent.starts_with(root.join("tmp").canonicalize()?) {
        return Err("fresh absolute workspace TMP output required".into());
    }
    let target = parent.join(path.file_name().ok_or("output file name missing")?);
    write_fresh_file(&target, bytes)
}

pub(in crate::markdown::svg_rasterize) fn write_fresh_file(
    path: &Path,
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    Ok(())
}

#[cfg(test)]
mod io_contract_tests {
    use super::{TempProbeDirectory, write_fresh_file};
    use std::{error::Error, io::ErrorKind};

    type ProbeResult<T> = Result<T, Box<dyn Error>>;

    #[test]
    fn existing_file_collision_preserves_original_bytes() -> ProbeResult<()> {
        let directory = TempProbeDirectory::create()?;
        let path = directory.file("occupied-output.bin");
        let original = b"original bytes";
        std::fs::write(&path, original)?;

        let error = write_fresh_file(&path, b"replacement bytes")
            .err()
            .ok_or("existing output was replaced")?;
        assert_io_error_kind(error, ErrorKind::AlreadyExists)?;
        assert_eq!(std::fs::read(path)?, original);
        Ok(())
    }

    #[test]
    fn missing_parent_is_not_created() -> ProbeResult<()> {
        let directory = TempProbeDirectory::create()?;
        let parent = directory.file("missing-parent");
        let path = parent.join("output.bin");
        assert!(!parent.exists());

        let error = write_fresh_file(&path, b"output bytes")
            .err()
            .ok_or("output was created without its parent")?;
        assert_io_error_kind(error, ErrorKind::NotFound)?;
        assert!(!parent.exists());
        assert!(!path.exists());
        Ok(())
    }

    fn assert_io_error_kind(error: Box<dyn Error>, expected: ErrorKind) -> ProbeResult<()> {
        let actual = error
            .downcast_ref::<std::io::Error>()
            .ok_or("output failure was not an I/O error")?;
        assert_eq!(actual.kind(), expected);
        Ok(())
    }
}
