use super::*;
use crate::renderer::backends::{
    HtmlArtifactProvenance, HtmlBrowserFrame, HtmlBrowserOrigin, HtmlBrowserPixelFormat,
    HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime,
};
use std::path::PathBuf;

const FINGERPRINT_HEX_LEN: usize = 64;
#[cfg(unix)]
const DENIED_FILE_MODE: u32 = 0o000;
const PARENT_TOC_VIEWPORT_WIDTH: u32 = 320;
const PARENT_TOC_VIEWPORT_HEIGHT: u32 = 180;
const PARENT_TOC_SCROLL_DELTA_Y: f32 = 120.0;
type ParentTocArtifact = (HtmlBrowserSource, HtmlBrowserViewport, HtmlFrameArtifact);

#[test]
fn real_store_roundtrip_missing_and_corrupt_paths() -> Result<(), Box<dyn std::error::Error>> {
    let (root, _owner) = test_root()?;
    let artifact = test_artifact()?;
    let store = HtmlArtifactStore::new(&root, MAX_ARTIFACT_BYTES, true)?;
    assert!(store.load("missing", &artifact.provenance)?.is_none());
    store.save("frame", &artifact)?;
    assert_eq!(
        store.load("frame", &artifact.provenance)?,
        Some(artifact.clone())
    );
    #[cfg(unix)]
    assert_load_permission_denied(&root, &store, &artifact)?;
    std::fs::write(root.join("frame.krr-frame.json"), b"not-json")?;
    assert!(matches!(
        store.load("frame", &artifact.provenance),
        Err(HtmlArtifactError::Corrupt)
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn store_rejects_root_symlink_before_creating_target() -> Result<(), Box<dyn std::error::Error>> {
    let (parent, _owner) = test_root()?;
    let target = parent.join("target");
    let alias = parent.join("alias");
    std::os::unix::fs::symlink(&target, &alias)?;

    assert!(matches!(
        HtmlArtifactStore::new(&alias, MAX_ARTIFACT_BYTES, true),
        Err(HtmlArtifactError::SymlinkPath)
    ));
    assert!(!target.exists());

    Ok(())
}

#[test]
fn nested_regular_files_count_toward_quota_and_preserve_old_artifact()
-> Result<(), Box<dyn std::error::Error>> {
    let (root, _owner) = test_root()?;
    let artifact = test_artifact()?;
    let bytes = serde_json::to_vec(&artifact)?;
    let store = HtmlArtifactStore::new(&root, bytes.len() as u64, true)?;
    store.save("frame", &artifact)?;
    let nested = root.join("nested");
    std::fs::create_dir(&nested)?;
    std::fs::write(nested.join("unknown.bin"), vec![0; 1])?;

    assert!(matches!(
        store.save("frame", &artifact),
        Err(HtmlArtifactError::CapacityExceeded)
    ));
    assert_eq!(store.load("frame", &artifact.provenance)?, Some(artifact));

    Ok(())
}

#[test]
fn stale_runtime_artifact_is_a_miss_and_stale_expected_is_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let (root, _owner) = test_root()?;
    let artifact = test_artifact()?;
    let store = HtmlArtifactStore::new(&root, MAX_ARTIFACT_BYTES, true)?;
    let mut stale = artifact.clone();
    stale.provenance.runtime_version = "0.0.0".into();
    std::fs::write(
        root.join("stale.krr-frame.json"),
        serde_json::to_vec(&stale)?,
    )?;

    assert!(store.load("stale", &artifact.provenance)?.is_none());
    assert!(matches!(
        store.load("stale", &stale.provenance),
        Err(HtmlArtifactError::InvalidProvenance(_))
    ));
    assert!(matches!(
        store.save("stale", &stale),
        Err(HtmlArtifactError::InvalidProvenance(_))
    ));

    Ok(())
}

#[test]
fn root_replacement_is_rejected_before_artifact_io() -> Result<(), Box<dyn std::error::Error>> {
    let (root, _owner) = test_root()?;
    let artifact = test_artifact()?;
    let store = HtmlArtifactStore::new(&root, MAX_ARTIFACT_BYTES, true)?;
    std::fs::remove_dir(&root)?;
    std::fs::write(&root, b"root replacement")?;

    assert!(matches!(
        store.save("frame", &artifact),
        Err(HtmlArtifactError::Io(_))
    ));
    assert!(matches!(
        store.load("frame", &artifact.provenance),
        Err(HtmlArtifactError::Io(_))
    ));

    Ok(())
}

fn test_root() -> std::io::Result<(PathBuf, tempfile::TempDir)> {
    let parent = std::fs::canonicalize(std::env::temp_dir())?;
    let owner = tempfile::Builder::new()
        .prefix("krr-store-real-")
        .tempdir_in(parent)?;
    let root = owner.path().join("store");
    std::fs::create_dir(&root)?;
    Ok((root, owner))
}

fn test_artifact() -> Result<HtmlFrameArtifact, Box<dyn std::error::Error>> {
    let origin = HtmlBrowserOrigin::parse("https://example.test/frame.html")?;
    let viewport = HtmlBrowserViewport::new(1, 1, 1.0)?;
    let provenance = HtmlArtifactProvenance {
        source_sha256: "a".repeat(FINGERPRINT_HEX_LEN),
        origin: origin.clone(),
        runtime_version: env!("CARGO_PKG_VERSION").to_owned(),
        viewport,
        render_config_fingerprint: "b".repeat(FINGERPRINT_HEX_LEN),
        resource_content_fingerprint: "c".repeat(FINGERPRINT_HEX_LEN),
        font_content_fingerprint: "d".repeat(FINGERPRINT_HEX_LEN),
    };
    let frame = HtmlBrowserFrame::new(
        1,
        origin,
        viewport,
        HtmlBrowserPixelFormat::Rgba8,
        vec![0; 4],
    )?;
    Ok(HtmlFrameArtifact::new(provenance, frame)?)
}

#[test]
fn parent_toc_store_roundtrip_rejects_source_and_artifact_tampering()
-> Result<(), Box<dyn std::error::Error>> {
    let (root, _owner) = test_root()?;
    let (source, viewport, artifact) = parent_toc_artifact()?;
    let store = HtmlArtifactStore::new(&root, MAX_ARTIFACT_BYTES, true)?;
    store.save("parent-toc", &artifact)?;
    let loaded = store
        .load("parent-toc", &artifact.provenance)?
        .ok_or("parent TOC artifact missing after save")?;
    assert_parent_toc_resume(&loaded, &source, viewport)?;
    assert_tampered_artifact_is_rejected(&root, &store, &artifact.provenance)?;
    assert_nonregular_store_entry_is_rejected(&root, &store, &artifact.provenance)?;
    #[cfg(unix)]
    assert_symlink_loop_is_rejected(&root, &store, &artifact)?;
    Ok(())
}

fn parent_toc_artifact() -> Result<ParentTocArtifact, Box<dyn std::error::Error>> {
    let source = HtmlBrowserSource::new(
        "<!doctype html><style>body{margin:0}nav{position:fixed;top:0;left:0}main{margin-top:32px;min-height:900px}</style><nav aria-label='Contents'><a href='#reservation'>Reservations</a></nav><main id='reservation'><h1>Library reservations</h1><div style='height:800px'>Reservation details</div></main>",
        "https://example.test/parent-toc.html",
    )?;
    let viewport =
        HtmlBrowserViewport::new(PARENT_TOC_VIEWPORT_WIDTH, PARENT_TOC_VIEWPORT_HEIGHT, 1.0)?;
    let provenance = HtmlArtifactProvenance::for_source(
        &source,
        viewport,
        "a".repeat(FINGERPRINT_HEX_LEN),
        "b".repeat(FINGERPRINT_HEX_LEN),
        "c".repeat(FINGERPRINT_HEX_LEN),
    )?;
    let mut session = HtmlRuntime.open(source.clone(), viewport)?;
    session.dispatch_input(crate::renderer::backends::HtmlBrowserInput::Scroll {
        delta_x: 0.0,
        delta_y: PARENT_TOC_SCROLL_DELTA_Y,
    })?;
    let frame = session
        .latest_frame()
        .ok_or("parent TOC frame missing")?
        .clone();
    session.close()?;
    Ok((source, viewport, HtmlFrameArtifact::new(provenance, frame)?))
}

fn assert_parent_toc_resume(
    artifact: &HtmlFrameArtifact,
    source: &HtmlBrowserSource,
    viewport: HtmlBrowserViewport,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut resumed = artifact.resume_session(source.clone(), viewport, &artifact.provenance)?;
    assert!(
        resumed
            .latest_frame()
            .is_some_and(|frame| frame.scroll_y > 0.0)
    );
    resumed.close()?;
    let changed_toc = HtmlBrowserSource::new(
        source.raw_html.replace("Reservations", "Catalog"),
        source.origin.as_str(),
    )?;
    assert!(matches!(
        artifact.resume_session(changed_toc, viewport, &artifact.provenance),
        Err(HtmlArtifactError::ProvenanceMismatch)
    ));
    Ok(())
}

fn assert_tampered_artifact_is_rejected(
    root: &std::path::Path,
    store: &HtmlArtifactStore,
    expected: &HtmlArtifactProvenance,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = root.join("parent-toc.krr-frame.json");
    let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let pixel = value["frame"]["pixels"][0]
        .as_u64()
        .ok_or("stored frame pixel missing")? as u8;
    value["frame"]["pixels"][0] = serde_json::json!(pixel ^ 1);
    std::fs::write(&path, serde_json::to_vec(&value)?)?;
    assert!(matches!(
        store.load("parent-toc", expected),
        Err(HtmlArtifactError::Corrupt)
    ));
    Ok(())
}

fn assert_nonregular_store_entry_is_rejected(
    root: &std::path::Path,
    store: &HtmlArtifactStore,
    expected: &HtmlArtifactProvenance,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = root.join("directory.krr-frame.json");
    std::fs::create_dir(&path)?;
    assert!(matches!(
        store.load("directory", expected),
        Err(HtmlArtifactError::Io(_))
    ));
    assert!(matches!(
        store.clear("directory"),
        Err(HtmlArtifactError::Io(_))
    ));
    std::fs::remove_dir(path)?;
    Ok(())
}

#[cfg(unix)]
fn assert_load_permission_denied(
    root: &std::path::Path,
    store: &HtmlArtifactStore,
    artifact: &HtmlFrameArtifact,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::{io::ErrorKind::PermissionDenied, os::unix::fs::PermissionsExt};
    let path = root.join("frame.krr-frame.json");
    let bytes = std::fs::read(&path)?;
    let original = std::fs::metadata(&path)?.permissions();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(DENIED_FILE_MODE))?;
    let result = store.load("frame", &artifact.provenance);
    std::fs::set_permissions(&path, original)?;
    let permission_denied =
        matches!(result, Err(HtmlArtifactError::Io(error)) if error.kind() == PermissionDenied);
    assert!(permission_denied);
    assert_eq!(std::fs::read(&path)?, bytes);
    Ok(())
}

#[cfg(unix)]
fn assert_symlink_loop_is_rejected(
    root: &std::path::Path,
    store: &HtmlArtifactStore,
    artifact: &HtmlFrameArtifact,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = root.join("loop.krr-frame.json");
    std::os::unix::fs::symlink(&path, &path)?;
    assert!(matches!(
        store.save("loop", artifact),
        Err(HtmlArtifactError::SymlinkPath)
    ));
    assert!(matches!(
        store.load("loop", &artifact.provenance),
        Err(HtmlArtifactError::SymlinkPath)
    ));
    std::fs::remove_file(path)?;
    Ok(())
}
