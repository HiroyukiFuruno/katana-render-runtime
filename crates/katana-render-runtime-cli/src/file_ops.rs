use anyhow::Context;
use std::path::PathBuf;

pub(crate) struct FileOps;

impl FileOps {
    pub(crate) fn read_to_string(path: &PathBuf) -> anyhow::Result<String> {
        std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
    }

    pub(crate) fn write(path: &PathBuf, bytes: &[u8]) -> anyhow::Result<()> {
        std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::FileOps;

    #[test]
    fn writes_and_reads_text() -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!("krr-file-{}.txt", std::process::id()));
        FileOps::write(&path, b"ok")?;
        assert_eq!(FileOps::read_to_string(&path)?, "ok");
        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn read_and_write_errors_keep_path_context() {
        let missing =
            std::env::temp_dir().join(format!("krr-file-missing-{}.txt", std::process::id()));
        let unwritable = std::env::temp_dir()
            .join(format!("krr-file-missing-dir-{}", std::process::id()))
            .join("out.txt");

        assert!(FileOps::read_to_string(&missing).is_err());
        assert!(FileOps::write(&unwritable, b"ok").is_err());
    }
}
