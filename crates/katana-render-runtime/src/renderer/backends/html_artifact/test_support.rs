use super::super::{
    HtmlArtifactError, HtmlArtifactProvenance, HtmlBrowserSource, HtmlBrowserViewport,
    HtmlFrameArtifact,
};
use super::{A, B, C};
use crate::{HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserOrigin, HtmlBrowserPixelFormat};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "test_support_interaction.rs"]
mod interaction;
#[path = "test_support_store.rs"]
mod store;

pub(super) use interaction::{exercise_resumed_interactions, save_initial_and_updated};
pub(super) use store::{
    assert_dangling_symlink_is_rejected, assert_explicit_lock_recovery,
    assert_hit_miss_and_corrupt, assert_stale_temp_is_preserved, assert_store_capacity_and_clear,
};

pub(super) fn assert_other_origin_rejected(
    viewport: HtmlBrowserViewport,
    provenance: HtmlArtifactProvenance,
) -> Result<(), Box<dyn std::error::Error>> {
    let other = HtmlBrowserOrigin::parse("https://example.test/other.html")?;
    let frame = HtmlBrowserFrame::new(
        1,
        other,
        viewport,
        HtmlBrowserPixelFormat::Rgba8,
        vec![0; 16],
    );
    assert!(
        frame.is_ok_and(|frame| matches!(
            HtmlFrameArtifact::new(provenance, frame),
            Err(HtmlArtifactError::ProvenanceMismatch)
        )),
        "valid foreign-origin frame must be rejected by artifact provenance"
    );
    Ok(())
}

pub(super) fn assert_invalid_resume_provenance(
    artifact: &HtmlFrameArtifact,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = source()?;
    let viewport = HtmlBrowserViewport::new(1, 1, 1.0)?;
    let mut invalid = artifact.provenance.clone();
    invalid.source_sha256 = "invalid".into();
    assert!(matches!(
        artifact.resume_session(source, viewport, &invalid),
        Err(HtmlArtifactError::InvalidProvenance(_))
    ));
    Ok(())
}

pub(super) fn source() -> Result<HtmlBrowserSource, HtmlBrowserError> {
    HtmlBrowserSource::new("<p>safe</p>", "https://example.test/a.html")
}
pub(super) fn provenance(
    source: &HtmlBrowserSource,
    viewport: HtmlBrowserViewport,
) -> Result<HtmlArtifactProvenance, HtmlArtifactError> {
    HtmlArtifactProvenance::for_source(source, viewport, A, B, C)
}
pub(super) fn artifact() -> Result<HtmlFrameArtifact, Box<dyn std::error::Error>> {
    let source = source()?;
    let viewport = HtmlBrowserViewport::new(1, 1, 1.0)?;
    let frame = HtmlBrowserFrame::new(
        1,
        source.origin.clone(),
        viewport,
        HtmlBrowserPixelFormat::Rgba8,
        vec![0; 4],
    )?;
    Ok(HtmlFrameArtifact::new(
        provenance(&source, viewport)?,
        frame,
    )?)
}
pub(super) fn root(label: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    Ok(std::fs::canonicalize(std::env::temp_dir())?.join(format!(
        "krr-artifact-{label}-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    )))
}
pub(super) fn html() -> &'static str {
    r#"<!doctype html><style>html,body{margin:0}body{min-height:600px;font:16px sans-serif}button,input{display:block;margin:8px;width:180px;height:28px}a{position:absolute;top:120px;left:8px;width:180px;height:28px}</style><button id="action">Run action</button><input id="entry"><p id="output">idle</p><a href="next.html">Next page</a><script>const o=document.querySelector('#output');document.querySelector('#action').addEventListener('click',()=>o.textContent='clicked');document.querySelector('#entry').addEventListener('input',e=>o.textContent=e.target.value);</script>"#
}
