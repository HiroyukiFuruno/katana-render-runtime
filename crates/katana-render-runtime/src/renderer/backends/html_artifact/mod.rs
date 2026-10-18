mod artifact;
mod encoding;
mod filesystem;
mod key;
mod store;
mod types;

pub use artifact::HtmlFrameArtifact;
pub use store::HtmlArtifactStore;
pub use types::{HtmlArtifactError, HtmlArtifactProvenance};

use super::html_browser::{
    HtmlBrowserInput, HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime,
};

impl HtmlFrameArtifact {
    pub fn resume_session(
        &self,
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
        expected: &HtmlArtifactProvenance,
    ) -> Result<HtmlBrowserSession, HtmlArtifactError> {
        self.validate()?;
        expected.validate()?;
        if expected.runtime_version != env!("CARGO_PKG_VERSION")
            || self.provenance != *expected
            || HtmlArtifactProvenance::source_sha256(&source) != expected.source_sha256
            || source.origin != expected.origin
            || viewport != expected.viewport
        {
            return Err(HtmlArtifactError::ProvenanceMismatch);
        }
        let mut session = HtmlRuntime
            .open(source, viewport)
            .map_err(|error| HtmlArtifactError::Runtime(error.to_string()))?;
        if self.frame.scroll_y > 0.0 {
            session
                .dispatch_input(HtmlBrowserInput::Scroll {
                    delta_x: 0.0,
                    delta_y: self.frame.scroll_y,
                })
                .map_err(|error| HtmlArtifactError::Runtime(error.to_string()))?;
        }
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::types::MAX_ARTIFACT_BYTES;
    use super::*;
    use crate::{HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserOrigin, HtmlBrowserPixelFormat};
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

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

    fn assert_other_origin_rejected(
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
    fn encode_store_and_resume_reject_invalid_contract_inputs()
    -> Result<(), Box<dyn std::error::Error>> {
        let artifact = artifact()?;
        assert!(matches!(
            artifact.encode_with_limit(1),
            Err(HtmlArtifactError::ArtifactTooLarge)
        ));
        let root = root("invalid-contract")?;
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
        fs::remove_dir_all(root)?;
        Ok(())
    }

    fn assert_invalid_resume_provenance(
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

    #[test]
    fn contract_rejects_source_runtime_capacity_and_clear() -> Result<(), Box<dyn std::error::Error>>
    {
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
    fn resume_session_reports_public_runtime_compile_error()
    -> Result<(), Box<dyn std::error::Error>> {
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

    fn assert_store_capacity_and_clear(
        artifact: &HtmlFrameArtifact,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = artifact.encode_with_limit(MAX_ARTIFACT_BYTES)?;
        let quota_root = root("quota")?;
        let store = HtmlArtifactStore::new(&quota_root, bytes.len() as u64, true)?;
        store.save("first", artifact)?;
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

    fn assert_disabled_store(
        artifact: &HtmlFrameArtifact,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let disabled_root = root("disabled")?;
        let disabled = HtmlArtifactStore::new(&disabled_root, 1_000, false)?;
        assert!(!disabled_root.exists());
        assert!(matches!(
            disabled.save("frame", artifact),
            Err(HtmlArtifactError::Disabled)
        ));
        Ok(())
    }

    #[test]
    fn store_hit_miss_corrupt_and_explicit_lock_recovery() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = root("store")?;
        let store = HtmlArtifactStore::new(&root, 1_000_000, true)?;
        let artifact = artifact()?;
        let expected = artifact.provenance.clone();
        assert_stale_temp_is_preserved(&root, &store, &artifact)?;
        assert_hit_miss_and_corrupt(&root, &store, &expected)?;
        assert_dangling_symlink_is_rejected(&root, &store, &expected)?;
        assert_explicit_lock_recovery(&root, &store, &artifact)?;
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    fn assert_stale_temp_is_preserved(
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

    fn assert_hit_miss_and_corrupt(
        root: &std::path::Path,
        store: &HtmlArtifactStore,
        expected: &HtmlArtifactProvenance,
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert!(store.load("frame", expected)?.is_some());
        let mut mismatch = expected.clone();
        mismatch.source_sha256 = "d".repeat(64);
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

    fn assert_dangling_symlink_is_rejected(
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

    fn assert_explicit_lock_recovery(
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
        let root = root("child")?;
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
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    fn save_initial_and_updated(
        store: &HtmlArtifactStore,
        provenance: &HtmlArtifactProvenance,
        session: &mut HtmlBrowserSession,
        initial: HtmlBrowserFrame,
    ) -> Result<(), Box<dyn std::error::Error>> {
        store.save(
            "child",
            &HtmlFrameArtifact::new(provenance.clone(), initial.clone())?,
        )?;
        click(session, 24.0, 24.0);
        let updated = session.latest_frame().ok_or("frame")?.clone();
        assert_ne!(updated.pixels, initial.pixels);
        store.save(
            "child",
            &HtmlFrameArtifact::new(provenance.clone(), updated)?,
        )?;
        Ok(())
    }

    fn exercise_resumed_interactions(
        resumed: &mut HtmlBrowserSession,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let before = resumed.latest_frame().ok_or("form")?.pixels.clone();
        click(resumed, 24.0, 60.0);
        resumed.dispatch_input(HtmlBrowserInput::Text { text: "ok".into() })?;
        assert_ne!(&resumed.latest_frame().ok_or("form")?.pixels, &before);
        click(resumed, 24.0, 130.0);
        assert!(resumed.take_navigation().is_some());
        resumed.dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 40.0,
        })?;
        assert!(resumed.latest_frame().ok_or("scroll")?.scroll_y > 0.0);
        Ok(())
    }

    fn click(session: &mut HtmlBrowserSession, x: f32, y: f32) {
        for input in [
            HtmlBrowserInput::PointerMove { x, y },
            HtmlBrowserInput::PointerDown { x, y, button: 0 },
            HtmlBrowserInput::PointerUp { x, y, button: 0 },
        ] {
            assert!(
                session.dispatch_input(input).is_ok(),
                "artifact fixture pointer input must succeed"
            );
        }
    }
    fn source() -> Result<HtmlBrowserSource, HtmlBrowserError> {
        HtmlBrowserSource::new("<p>safe</p>", "https://example.test/a.html")
    }
    fn provenance(
        source: &HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<HtmlArtifactProvenance, HtmlArtifactError> {
        HtmlArtifactProvenance::for_source(source, viewport, A, B, C)
    }
    fn artifact() -> Result<HtmlFrameArtifact, Box<dyn std::error::Error>> {
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
    fn root(label: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        Ok(std::fs::canonicalize(std::env::temp_dir())?.join(format!(
            "krr-artifact-{label}-{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        )))
    }
    fn html() -> &'static str {
        r#"<!doctype html><style>html,body{margin:0}body{min-height:600px;font:16px sans-serif}button,input{display:block;margin:8px;width:180px;height:28px}a{position:absolute;top:120px;left:8px;width:180px;height:28px}</style><button id="action">Run action</button><input id="entry"><p id="output">idle</p><a href="next.html">Next page</a><script>const o=document.querySelector('#output');document.querySelector('#action').addEventListener('click',()=>o.textContent='clicked');document.querySelector('#entry').addEventListener('input',e=>o.textContent=e.target.value);</script>"#
    }
}
