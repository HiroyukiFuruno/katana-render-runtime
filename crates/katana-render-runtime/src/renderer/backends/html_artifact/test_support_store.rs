use super::super::super::types::MAX_ARTIFACT_BYTES;
use super::super::super::{
    HtmlArtifactError, HtmlArtifactProvenance, HtmlArtifactStore, HtmlFrameArtifact,
};
use super::root;
use std::fs;

const DISABLED_STORE_CAPACITY: u64 = 1_000;
const SHA256_HEX_LENGTH: usize = 64;

pub(in super::super) fn assert_store_capacity_and_clear(
    artifact: &HtmlFrameArtifact,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = artifact.encode_with_limit(MAX_ARTIFACT_BYTES)?;
    let quota_root = root("quota")?;
    let store = HtmlArtifactStore::new(&quota_root, bytes.len() as u64, true)?;
    store.save("first", artifact)?;
    store.save("first", artifact)?;
    assert!(store.load("first", &artifact.provenance)?.is_some());
    let limited = HtmlArtifactStore::new(&quota_root, bytes.len() as u64 - 1, true)?;
    assert!(matches!(
        limited.save("too-small", artifact),
        Err(HtmlArtifactError::CapacityExceeded)
    ));
    assert!(matches!(
        limited.load("first", &artifact.provenance),
        Err(HtmlArtifactError::CapacityExceeded)
    ));
    assert!(matches!(
        store.save("second", artifact),
        Err(HtmlArtifactError::CapacityExceeded)
    ));
    assert!(store.clear("first")?);
    assert!(!store.clear("first")?);
    assert_disabled_store(artifact)?;
    Ok(())
}

fn assert_disabled_store(artifact: &HtmlFrameArtifact) -> Result<(), Box<dyn std::error::Error>> {
    let disabled_root = root("disabled")?;
    let disabled = HtmlArtifactStore::new(&disabled_root, DISABLED_STORE_CAPACITY, false)?;
    assert!(!disabled_root.exists());
    assert!(matches!(
        disabled.save("frame", artifact),
        Err(HtmlArtifactError::Disabled)
    ));
    Ok(())
}

pub(in super::super) fn assert_stale_temp_is_preserved(
    root: &std::path::Path,
    store: &HtmlArtifactStore,
    artifact: &HtmlFrameArtifact,
) -> Result<(), Box<dyn std::error::Error>> {
    let stale_temp = root.join(format!("frame.krr-frame.tmp-{}", std::process::id()));
    fs::write(&stale_temp, b"previous temp")?;
    assert!(matches!(
        store.save("frame", artifact),
        Err(HtmlArtifactError::Io(_))
    ));
    assert!(stale_temp.is_file());
    fs::remove_file(&stale_temp)?;
    store.save("frame", artifact)?;
    Ok(())
}

pub(in super::super) fn assert_hit_miss_and_corrupt(
    root: &std::path::Path,
    store: &HtmlArtifactStore,
    expected: &HtmlArtifactProvenance,
) -> Result<(), Box<dyn std::error::Error>> {
    assert!(store.load("frame", expected)?.is_some());
    let mut mismatch = expected.clone();
    mismatch.source_sha256 = "d".repeat(SHA256_HEX_LENGTH);
    assert!(store.load("frame", &mismatch)?.is_none());
    fs::write(root.join("frame.krr-frame.json"), b"{}")?;
    assert!(matches!(
        store.load("frame", expected),
        Err(HtmlArtifactError::Corrupt)
    ));
    let blocked = root.join("blocked.krr-frame.json");
    fs::create_dir(&blocked)?;
    assert!(matches!(
        store.load("blocked", expected),
        Err(HtmlArtifactError::Io(_))
    ));
    assert!(matches!(
        store.clear("blocked"),
        Err(HtmlArtifactError::Io(_))
    ));
    fs::remove_dir(blocked)?;
    Ok(())
}

pub(in super::super) fn assert_dangling_symlink_is_rejected(
    root: &std::path::Path,
    store: &HtmlArtifactStore,
    expected: &HtmlArtifactProvenance,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            "/missing-krr-artifact",
            root.join("dangling.krr-frame.json"),
        )?;
        assert!(matches!(
            store.load("dangling", expected),
            Err(HtmlArtifactError::SymlinkPath)
        ));
    }
    Ok(())
}

pub(in super::super) fn assert_explicit_lock_recovery(
    root: &std::path::Path,
    store: &HtmlArtifactStore,
    artifact: &HtmlFrameArtifact,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(root.join(".krr-artifact.lock"), b"crashed")?;
    assert!(matches!(
        store.save("frame", artifact),
        Err(HtmlArtifactError::StoreBusy)
    ));
    assert!(store.recover_lock()?);
    assert!(!store.recover_lock()?);
    fs::create_dir(root.join(".krr-artifact.lock"))?;
    assert!(matches!(
        store.recover_lock(),
        Err(HtmlArtifactError::Io(_))
    ));
    fs::remove_dir(root.join(".krr-artifact.lock"))?;
    Ok(())
}
