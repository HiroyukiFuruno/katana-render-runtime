use super::{pixel_sha, raster_observer, record_phase};
use crate::{
    HtmlBrowserInput, HtmlBrowserSession, HtmlBrowserSessionState, HtmlBrowserSource,
    HtmlBrowserViewport, HtmlRuntime,
};

const SCROLL_STEP: f32 = 360.0;

pub(super) fn cold_open(
    source: &impl Fn() -> Result<HtmlBrowserSource, String>,
    viewport: HtmlBrowserViewport,
) -> Result<HtmlBrowserSession, String> {
    let observer = raster_observer::Scope::enter();
    let input = source()?;
    observe_open(&input, viewport);
    let session = HtmlRuntime
        .open(input, viewport)
        .map_err(|e| e.to_string())?;
    record_phase("cold_open", &session, &observer)?;
    drop(observer);
    Ok(session)
}

pub(super) fn warm_open(
    source: &impl Fn() -> Result<HtmlBrowserSource, String>,
    viewport: HtmlBrowserViewport,
) -> Result<(), String> {
    let observer = raster_observer::Scope::enter();
    let input = source()?;
    observe_open(&input, viewport);
    let mut warm = HtmlRuntime
        .open(input, viewport)
        .map_err(|e| e.to_string())?;
    record_phase("warm_open", &warm, &observer)?;
    close_control(&mut warm, "warm_close")?;
    assert_eq!(warm.state(), HtmlBrowserSessionState::Closed);
    Ok(())
}

pub(super) fn re_display(session: &mut HtmlBrowserSession) -> Result<(), String> {
    let observer = raster_observer::Scope::enter();
    raster_observer::work_value("refresh", serde_json::json!({"action": "refresh_frame"}));
    session.refresh_frame().map_err(|e| e.to_string())?;
    record_phase("re_display", session, &observer)
}

pub(super) fn max_scroll(
    session: &HtmlBrowserSession,
    viewport: HtmlBrowserViewport,
) -> Result<f32, String> {
    Ok((session
        .latest_frame()
        .ok_or("initial frame missing")?
        .content_height
        .round()
        - viewport.logical_height())
    .max(0.0))
}

pub(super) fn scroll(delta_y: f32) -> HtmlBrowserInput {
    HtmlBrowserInput::Scroll {
        delta_x: 0.0,
        delta_y,
    }
}

pub(super) fn full_y(session: &mut HtmlBrowserSession, max_scroll: f32) -> Result<(), String> {
    let observer = raster_observer::Scope::enter();
    let mut previous = session
        .latest_frame()
        .ok_or("scroll frame missing")?
        .scroll_y;
    while previous < max_scroll {
        raster_observer::work_value(
            "scroll_iteration",
            serde_json::json!({"iterationStartScrollBits": previous.to_bits()}),
        );
        dispatch(session, scroll(SCROLL_STEP))?;
        let next = session
            .latest_frame()
            .ok_or("scroll frame missing")?
            .scroll_y;
        assert!(next > previous, "scroll must progress to the bottom");
        previous = next;
    }
    assert_eq!(previous, max_scroll);
    record_phase("full_y", session, &observer)
}

pub(super) fn post_operation(session: &mut HtmlBrowserSession) -> Result<(), String> {
    let before = session.latest_frame().ok_or("operation frame missing")?;
    assert_eq!(before.scroll_y, 0.0);
    let generation = before.generation;
    let pixels = pixel_sha(&before.pixels);
    {
        let observer = raster_observer::Scope::enter();
        let (x, y) = (100.0, 100.0);
        for input in [
            HtmlBrowserInput::PointerMove { x, y },
            HtmlBrowserInput::PointerDown { x, y, button: 0 },
            HtmlBrowserInput::PointerUp { x, y, button: 0 },
        ] {
            dispatch(session, input)?;
        }
        let after = session.latest_frame().ok_or("changed frame missing")?;
        assert!(after.generation > generation);
        assert_ne!(pixel_sha(&after.pixels), pixels);
        record_phase("post_operation", session, &observer)?;
    }
    Ok(())
}

fn observe_open(source: &HtmlBrowserSource, viewport: HtmlBrowserViewport) {
    use sha2::{Digest, Sha256};
    raster_observer::work_value(
        "open",
        serde_json::json!({"sourceSha256":
        Sha256::digest(source.raw_html.as_bytes()).to_vec(), "sourceBytes": source.raw_html.len(),
        "origin": source.origin.as_str(), "viewport": viewport}),
    );
}

fn dispatch(session: &mut HtmlBrowserSession, input: HtmlBrowserInput) -> Result<(), String> {
    raster_observer::work_value("action", serde_json::json!({"input": &input}));
    session.dispatch_input(input).map_err(|e| e.to_string())
}

pub(super) fn scroll_top_control(
    session: &mut HtmlBrowserSession,
    max_scroll: f32,
) -> Result<(), String> {
    let observer = raster_observer::Scope::enter();
    dispatch(session, scroll(-max_scroll))?;
    let snapshot = observer.snapshot();
    println!(
        "KRR_PUBLIC_FIXTURE_FONT_CONTROL {}",
        snapshot.operation_counters()
    );
    assert!(
        snapshot.complete(),
        "control action and frame endpoints must be complete"
    );
    Ok(())
}

pub(super) fn close_control(session: &mut HtmlBrowserSession, control: &str) -> Result<(), String> {
    let observer = raster_observer::Scope::enter();
    raster_observer::work_value("close", serde_json::json!({"control": control}));
    session.close().map_err(|e| e.to_string())?;
    let snapshot = observer.snapshot();
    println!(
        "KRR_PUBLIC_FIXTURE_FONT_CONTROL {}",
        snapshot.operation_counters()
    );
    assert!(snapshot.complete(), "close control must be complete");
    Ok(())
}
