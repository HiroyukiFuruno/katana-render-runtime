use super::super::super::{
    HtmlArtifactProvenance, HtmlArtifactStore, HtmlBrowserInput, HtmlBrowserSession,
    HtmlFrameArtifact,
};
use crate::HtmlBrowserFrame;

const INTERACTION_X: f32 = 24.0;
const ACTION_BUTTON_Y: f32 = 24.0;
const TEXT_INPUT_Y: f32 = 60.0;
const LINK_Y: f32 = 130.0;
const SCROLL_DISTANCE_Y: f32 = 40.0;

pub(in super::super) fn save_initial_and_updated(
    store: &HtmlArtifactStore,
    provenance: &HtmlArtifactProvenance,
    session: &mut HtmlBrowserSession,
    initial: HtmlBrowserFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    store.save(
        "child",
        &HtmlFrameArtifact::new(provenance.clone(), initial.clone())?,
    )?;
    click(session, INTERACTION_X, ACTION_BUTTON_Y);
    let updated = session.latest_frame().ok_or("frame")?.clone();
    assert_ne!(updated.pixels, initial.pixels);
    store.save(
        "child",
        &HtmlFrameArtifact::new(provenance.clone(), updated)?,
    )?;
    Ok(())
}

pub(in super::super) fn exercise_resumed_interactions(
    resumed: &mut HtmlBrowserSession,
) -> Result<(), Box<dyn std::error::Error>> {
    let before = resumed.latest_frame().ok_or("form")?.pixels.clone();
    click(resumed, INTERACTION_X, TEXT_INPUT_Y);
    resumed.dispatch_input(HtmlBrowserInput::Text { text: "ok".into() })?;
    assert_ne!(&resumed.latest_frame().ok_or("form")?.pixels, &before);
    click(resumed, INTERACTION_X, LINK_Y);
    assert!(resumed.take_navigation().is_some());
    resumed.dispatch_input(HtmlBrowserInput::Scroll {
        delta_x: 0.0,
        delta_y: SCROLL_DISTANCE_Y,
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

#[cfg(test)]
mod tests {
    use super::{exercise_resumed_interactions, save_initial_and_updated};
    use crate::HtmlBrowserFrame;
    use crate::renderer::backends::html_artifact::types::MAX_ARTIFACT_BYTES;
    use crate::renderer::backends::html_artifact::{
        HtmlArtifactError, HtmlArtifactProvenance, HtmlArtifactStore, HtmlBrowserSource,
        HtmlBrowserViewport, HtmlFrameArtifact, HtmlRuntime,
    };
    use std::path::Path;

    #[test]
    fn resumed_interactions_reject_a_closed_session() -> Result<(), Box<dyn std::error::Error>> {
        let source = HtmlBrowserSource::new(
            super::super::html(),
            "https://example.test/closed-interaction.html",
        )?;
        let viewport = HtmlBrowserViewport::new(320, 160, 1.0)?;
        let mut session = HtmlRuntime.open(source, viewport)?;
        assert!(session.latest_frame().is_some());
        session.close()?;
        let error = exercise_resumed_interactions(&mut session)
            .err()
            .ok_or("closed session was accepted")?;
        assert_eq!(error.to_string(), "form");
        Ok(())
    }

    #[test]
    fn updated_frame_over_capacity_preserves_the_saved_initial_frame()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = quota_source()?;
        let viewport = HtmlBrowserViewport::new(320, 160, 1.0)?;
        let provenance = super::super::provenance(&source, viewport)?;
        let mut session = HtmlRuntime.open(source, viewport)?;
        let initial = session.latest_frame().ok_or("initial frame")?.clone();
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let store = store_with_initial_frame_capacity(&root, &provenance, &initial)?;
        let error = save_initial_and_updated(&store, &provenance, &mut session, initial.clone())
            .err()
            .ok_or("larger updated frame was accepted")?;
        assert!(matches!(
            error.downcast_ref(),
            Some(HtmlArtifactError::CapacityExceeded)
        ));
        let saved = store
            .load("child", &provenance)?
            .ok_or("saved initial frame")?;
        assert_eq!(saved.frame, initial);
        assert_ne!(
            session.latest_frame().ok_or("updated frame")?.pixels,
            initial.pixels
        );
        session.close()?;
        Ok(())
    }

    fn store_with_initial_frame_capacity(
        root: &Path,
        provenance: &HtmlArtifactProvenance,
        initial: &HtmlBrowserFrame,
    ) -> Result<HtmlArtifactStore, HtmlArtifactError> {
        let artifact = HtmlFrameArtifact::new(provenance.clone(), initial.clone())?;
        let capacity = artifact.encode_with_limit(MAX_ARTIFACT_BYTES)?.len() as u64;
        HtmlArtifactStore::new(root, capacity, true)
    }

    fn quota_source() -> Result<HtmlBrowserSource, Box<dyn std::error::Error>> {
        Ok(HtmlBrowserSource::new(
            "<style>body{margin:0;background:black}button{position:absolute;left:0;top:0;width:80px;height:50px}</style><button onclick=\"document.body.style.background='white'\">Change</button>",
            "https://example.test/quota.html",
        )?)
    }
}
