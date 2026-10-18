use std::path::{Path, PathBuf};

pub(crate) type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(crate) fn write_file(root: &Path, relative: &str, content: &str) -> TestResult<PathBuf> {
    let path = root.join(relative);
    let Some(parent) = path.parent() else {
        return Err(Box::new(std::io::Error::other("path has no parent")));
    };
    std::fs::create_dir_all(parent)?;
    std::fs::write(&path, content)?;
    Ok(path)
}

pub(crate) fn temp_root(prefix: &str, name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{name}-{}", std::process::id()))
}
