use super::types::MAX_ARTIFACT_BYTES;
use super::{
    HtmlArtifactError, HtmlArtifactProvenance, HtmlArtifactStore, HtmlBrowserInput,
    HtmlBrowserSource, HtmlBrowserViewport, HtmlFrameArtifact, HtmlRuntime,
};
use crate::{HtmlBrowserFrame, HtmlBrowserPixelFormat};
use std::process::Command;

#[path = "test_support.rs"]
mod test_support;
use test_support::{
    artifact, assert_dangling_symlink_is_rejected, assert_explicit_lock_recovery,
    assert_hit_miss_and_corrupt, assert_invalid_resume_provenance, assert_other_origin_rejected,
    assert_stale_temp_is_preserved, assert_store_capacity_and_clear, exercise_resumed_interactions,
    html, provenance, root, save_initial_and_updated, source,
};

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const SAVE_TEST_WIDTH: u32 = 320;
const SAVE_TEST_HEIGHT: u32 = 160;
const SAVE_TEST_STORE_CAPACITY: u64 = 8_000_000;

#[test]
fn provenance_and_frame_mutations_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let source = source()?;
    let viewport = HtmlBrowserViewport::new(2, 2, 1.0)?;
    let provenance = provenance(&source, viewport)?;
    assert!(
        HtmlArtifactProvenance {
            resource_content_fingerprint: String::new(),
            ..provenance.clone()
        }
        .validate()
        .is_err()
    );
    assert_other_origin_rejected(viewport, provenance)
}

#[test]
fn frame_pixel_mutation_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut changed = artifact()?;
    changed.frame.pixels[0] = 1;
    assert!(matches!(
        changed.validate(),
        Err(HtmlArtifactError::Corrupt)
    ));
    Ok(())
}

#[test]
fn resume_rejects_corrupt_saved_frame_before_opening_runtime()
-> Result<(), Box<dyn std::error::Error>> {
    let mut saved = artifact()?;
    let expected = saved.provenance.clone();
    let viewport = expected.viewport;
    saved.frame.pixels[0] ^= 1;
    assert!(matches!(
        saved.resume_session(source()?, viewport, &expected),
        Err(HtmlArtifactError::Corrupt)
    ));
    let mut unsupported = artifact()?;
    unsupported.format_version = u16::MAX;
    assert!(matches!(
        unsupported.resume_session(source()?, viewport, &expected),
        Err(HtmlArtifactError::UnsupportedFormatVersion(u16::MAX))
    ));
    Ok(())
}

#[test]
fn encode_store_and_resume_reject_invalid_contract_inputs() -> Result<(), Box<dyn std::error::Error>>
{
    let artifact = artifact()?;
    assert!(matches!(
        artifact.encode_with_limit(1),
        Err(HtmlArtifactError::ArtifactTooLarge)
    ));
    let (_directory, root) = root("invalid-contract")?;
    let store = HtmlArtifactStore::new(&root, MAX_ARTIFACT_BYTES, true)?;
    assert!(matches!(
        store.save("../escape", &artifact),
        Err(HtmlArtifactError::InvalidKey)
    ));
    assert!(matches!(
        store.load("../escape", &artifact.provenance),
        Err(HtmlArtifactError::InvalidKey)
    ));
    assert_invalid_resume_provenance(&artifact)?;
    Ok(())
}

#[test]
fn contract_rejects_source_runtime_capacity_and_clear() -> Result<(), Box<dyn std::error::Error>> {
    let source = source()?;
    let viewport = HtmlBrowserViewport::new(1, 1, 1.0)?;
    let provenance = provenance(&source, viewport)?;
    let artifact = artifact()?;
    let mut changed_source = source.clone();
    changed_source.raw_html = "<p>changed</p>".into();
    assert!(matches!(
        artifact.resume_session(changed_source, viewport, &provenance),
        Err(HtmlArtifactError::ProvenanceMismatch)
    ));
    let mut old_runtime = provenance.clone();
    old_runtime.runtime_version = "0.0.0".into();
    assert!(matches!(
        artifact.resume_session(source, viewport, &old_runtime),
        Err(HtmlArtifactError::ProvenanceMismatch)
    ));
    assert_store_capacity_and_clear(&artifact)?;
    Ok(())
}

#[test]
fn resume_session_restores_saved_scroll_position() -> Result<(), Box<dyn std::error::Error>> {
    let source = HtmlBrowserSource::new(
        "<div style='height:500px'>library reservation details</div>",
        "https://example.test/scroll.html",
    )?;
    let viewport = HtmlBrowserViewport::new(320, 160, 1.0)?;
    let provenance = provenance(&source, viewport)?;
    let mut session = HtmlRuntime.open(source.clone(), viewport)?;
    session.dispatch_input(HtmlBrowserInput::Scroll {
        delta_x: 0.0,
        delta_y: 120.0,
    })?;
    let frame = session.latest_frame().ok_or("frame")?.clone();
    assert!(frame.scroll_y > 0.0);
    let artifact = HtmlFrameArtifact::new(provenance.clone(), frame)?;
    session.close()?;

    let mut resumed = artifact.resume_session(source, viewport, &provenance)?;
    assert!(resumed.latest_frame().ok_or("resumed frame")?.scroll_y > 0.0);
    resumed.close()?;
    Ok(())
}

#[test]
fn resume_reports_scroll_listener_exception() -> Result<(), Box<dyn std::error::Error>> {
    let html = r#"<style>html,body{margin:0}</style><button id="allow" style="width:100px;height:40px">Allow</button><div style="height:500px"></div><script>
let allowed = false;
document.getElementById('allow').addEventListener('click', () => { allowed = true; });
window.addEventListener('scroll', () => { if (!allowed) throw new Error('resume scroll listener failed'); });
</script>"#;
    let source = HtmlBrowserSource::new(html, "https://example.test/scroll-error.html")?;
    let viewport = HtmlBrowserViewport::new(320, 160, 1.0)?;
    let provenance = provenance(&source, viewport)?;
    let mut session = HtmlRuntime.open(source.clone(), viewport)?;
    allow_and_scroll(&mut session)?;
    let frame = session.latest_frame().ok_or("frame")?.clone();
    assert!(frame.scroll_y > 0.0);
    let artifact = HtmlFrameArtifact::new(provenance.clone(), frame)?;
    session.close()?;
    assert!(matches!(
        artifact.resume_session(source, viewport, &provenance),
        Err(HtmlArtifactError::Runtime(message)) if message.contains("resume scroll listener failed")
    ));
    Ok(())
}

fn allow_and_scroll(
    session: &mut crate::HtmlBrowserSession,
) -> Result<(), Box<dyn std::error::Error>> {
    const BUTTON_POSITION: (f32, f32) = (10.0, 10.0);
    const SAVED_SCROLL_DISTANCE: f32 = 120.0;
    let (x, y) = BUTTON_POSITION;
    for input in [
        HtmlBrowserInput::PointerMove { x, y },
        HtmlBrowserInput::PointerDown { x, y, button: 0 },
        HtmlBrowserInput::PointerUp { x, y, button: 0 },
        HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: SAVED_SCROLL_DISTANCE,
        },
    ] {
        session.dispatch_input(input)?;
    }
    Ok(())
}

#[test]
fn resume_session_reports_public_runtime_compile_error() -> Result<(), Box<dyn std::error::Error>> {
    let source = HtmlBrowserSource::new(
        "<script>if (</script>",
        "https://example.test/invalid-script.html",
    )?;
    let viewport = HtmlBrowserViewport::new(1, 1, 1.0)?;
    let frame = HtmlBrowserFrame::new(
        1,
        source.origin.clone(),
        viewport,
        HtmlBrowserPixelFormat::Rgba8,
        vec![0; 4],
    )?;
    let provenance = HtmlArtifactProvenance::for_source(&source, viewport, A, B, C)?;
    let artifact = HtmlFrameArtifact::new(provenance.clone(), frame)?;

    assert!(matches!(
        artifact.resume_session(source, viewport, &provenance),
        Err(HtmlArtifactError::Runtime(message))
            if message.contains("JavaScript compile error")
    ));
    Ok(())
}

#[test]
fn store_hit_miss_corrupt_and_explicit_lock_recovery() -> Result<(), Box<dyn std::error::Error>> {
    let (_directory, root) = root("store")?;
    let store = HtmlArtifactStore::new(&root, 1_000_000, true)?;
    let artifact = artifact()?;
    let expected = artifact.provenance.clone();
    assert_stale_temp_is_preserved(&root, &store, &artifact)?;
    assert_hit_miss_and_corrupt(&root, &store, &expected)?;
    assert_dangling_symlink_is_rejected(&root, &store, &expected)?;
    assert_explicit_lock_recovery(&root, &store, &artifact)?;
    Ok(())
}

#[test]
fn independent_process_resume_exercises_js_form_link_scroll()
-> Result<(), Box<dyn std::error::Error>> {
    const ENV: &str = "KRR_HTML_ARTIFACT_CHILD";
    const NAME: &str = "renderer::backends::html_artifact::tests::artifact_child";
    let status = Command::new(std::env::current_exe()?)
        .args(["--exact", NAME, "--nocapture"])
        .env(ENV, "1")
        .status()?;
    assert!(status.success());
    Ok(())
}

#[test]
fn artifact_child() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("KRR_HTML_ARTIFACT_CHILD").is_none() {
        return Ok(());
    }
    let (_directory, root) = root("child")?;
    let store = HtmlArtifactStore::new(&root, 8_000_000, true)?;
    let source = HtmlBrowserSource::new(html(), "https://example.test/child.html")?;
    let viewport = HtmlBrowserViewport::new(320, 160, 1.0)?;
    let provenance = provenance(&source, viewport)?;
    let mut session = HtmlRuntime.open(source.clone(), viewport)?;
    let initial = session.latest_frame().ok_or("frame")?.clone();
    save_initial_and_updated(&store, &provenance, &mut session, initial)?;
    let mut resumed = store
        .load("child", &provenance)?
        .ok_or("hit")?
        .resume_session(source, viewport, &provenance)?;
    exercise_resumed_interactions(&mut resumed)?;
    resumed.close()?;
    session.close()?;
    Ok(())
}

#[rstest::rstest]
#[case::invalid_provenance(true)]
#[case::disabled_store(false)]
fn saving_interactive_frame_rejects_invalid_provenance_and_disabled_store(
    #[case] enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let source = HtmlBrowserSource::new(html(), "https://example.test/child.html")?;
    let viewport = HtmlBrowserViewport::new(SAVE_TEST_WIDTH, SAVE_TEST_HEIGHT, 1.0)?;
    let mut provenance = provenance(&source, viewport)?;
    let mut session = HtmlRuntime.open(source, viewport)?;
    let initial = session.latest_frame().ok_or("frame")?.clone();
    let root = temp.path().canonicalize()?;
    let store = HtmlArtifactStore::new(&root, SAVE_TEST_STORE_CAPACITY, enabled)?;
    if enabled {
        provenance.source_sha256 = "invalid".into();
    }
    let generation = initial.generation;
    let error = save_initial_and_updated(&store, &provenance, &mut session, initial)
        .err()
        .ok_or("unexpectedly saved the initial frame")?;
    assert!(matches!(
        (enabled, error.downcast_ref()),
        (true, Some(HtmlArtifactError::InvalidProvenance(_)))
            | (false, Some(HtmlArtifactError::Disabled))
    ));
    assert!(!temp.path().join("child.krr-frame.json").exists());
    assert_eq!(
        session.latest_frame().ok_or("frame")?.generation,
        generation
    );
    session.close().map_err(Into::into)
}
