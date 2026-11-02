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
