use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub(crate) struct TempProbeDirectory {
    root: PathBuf,
    _owner: tempfile::TempDir,
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
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    file.write_all(bytes)?;
    Ok(())
}
