use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_TEMP_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct TempProbeDirectory(PathBuf);

impl TempProbeDirectory {
    pub(crate) fn create() -> Result<Self, Box<dyn std::error::Error>> {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest
            .parent()
            .and_then(|path| path.parent())
            .ok_or("workspace root missing")?;
        let temp_root = root.join("tmp/html-fidelity");
        std::fs::create_dir_all(&temp_root)?;
        let path = temp_root.join(format!(
            "text-probe-test-{}-{}",
            std::process::id(),
            NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }

    pub(crate) fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
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

impl Drop for TempProbeDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
