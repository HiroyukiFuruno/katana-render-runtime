#[cfg(unix)]
use super::read_bounded_artifact;
use super::{
    HtmlArtifactError, HtmlArtifactStore, MAX_ARTIFACT_BYTES, map_save_error, size_limit_error,
};
use std::fs;

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

    const STORE_CAPACITY: u64 = 8;
    let root = tempfile::tempdir()?;
    let fifo = root.path().join("frame");
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
    let result = read_bounded_artifact(reader, STORE_CAPACITY);
    writer.join().map_err(|_| "FIFO writer panicked")??;
    assert!(matches!(result, Err(HtmlArtifactError::CapacityExceeded)));
    Ok(())
}
