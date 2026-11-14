use super::*;
use crate::renderer::backends::html_subresources::resource_audit::{Inventory, ResourceAuditScope};
use image::{Rgba, RgbaImage};
use std::path::Path;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const HTML: &str = r#"<!doctype html><link rel="stylesheet" href="style.css">
<div id="panel"></div><img src="image.png" width="16" height="16">
<p id="state">initial</p><script>window.resourceSentinel = 37;</script><div id="tail"></div>"#;
const RGBA_CHANNELS: usize = 4;
const TAIL_HEIGHT_PX: usize = 512;
const SMALL_SCROLL_DELTA: f32 = 1.0;
const EXPECTED_RESOURCE_COUNT: usize = 2;
const INITIAL_SENTINEL: i32 = 37;
const RETAINED_SENTINEL: i32 = 91;
const GREEN: [u8; RGBA_CHANNELS] = [0, 160, 0, 255];
const BLUE: [u8; RGBA_CHANNELS] = [0, 0, 255, 255];
const RED: [u8; RGBA_CHANNELS] = [255, 0, 0, 255];

fn css(root: &Path, color: &str) -> TestResult {
    std::fs::write(
        root.join("style.css"),
        format!(
            "html,body{{margin:0}} #panel{{width:16px;height:16px;background:{color}}} #tail{{height:{TAIL_HEIGHT_PX}px}}"
        ),
    )?;
    Ok(())
}

fn source(root: &Path) -> TestResult<HtmlBrowserSource> {
    let path = root.join("index.html");
    std::fs::write(&path, HTML)?;
    let origin = url::Url::from_file_path(path).map_err(|()| "invalid fixture origin")?;
    Ok(HtmlBrowserSource::new(HTML, origin.as_str())?)
}

fn audit(scope: ResourceAuditScope, root: &Path, name: &str) -> TestResult<Inventory> {
    scope.finish(&root.join(name)).map_err(Into::into)
}

fn assert_color(frame: &HtmlBrowserFrame, color: [u8; RGBA_CHANNELS], present: bool) {
    assert_eq!(
        frame
            .pixels
            .chunks_exact(RGBA_CHANNELS)
            .any(|pixel| pixel == color),
        present
    );
}

fn assert_frame_and_state(
    session: &mut HtmlInteractiveSession,
    viewport: HtmlBrowserViewport,
) -> TestResult {
    let frame = session.latest_frame().ok_or("missing frame")?;
    assert_eq!(frame.viewport, viewport);
    assert_eq!(
        frame.pixels.len(),
        viewport.width as usize * viewport.height as usize * RGBA_CHANNELS
    );
    assert_color(frame, GREEN, true);
    assert_color(frame, RED, true);
    assert_color(frame, BLUE, false);
    let state = session
        .runtime
        .snapshot()
        .map_err(|error| error.to_string())?;
    assert!(state.contains("retained"));
    let sentinel = session
        .runtime
        .observe_feature_state("window.resourceSentinel")?;
    assert_eq!(sentinel, RETAINED_SENTINEL);
    Ok(())
}

fn scroll_retained_session(
    session: &mut HtmlInteractiveSession,
    viewport: HtmlBrowserViewport,
) -> TestResult {
    session.dispatch_input(
        crate::renderer::backends::html_browser::HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: SMALL_SCROLL_DELTA,
        },
    )?;
    assert_frame_and_state(session, viewport)?;
    assert_eq!(
        session
            .latest_frame()
            .ok_or("missing scrolled frame")?
            .scroll_y,
        SMALL_SCROLL_DELTA
    );
    Ok(())
}

fn assert_initial_inventory(inventory: Inventory, root: &Path) -> TestResult {
    assert!(inventory.complete, "{}", inventory.reason);
    let requests = inventory.requests.as_array().ok_or("missing requests")?;
    assert_eq!(requests.len(), EXPECTED_RESOURCE_COUNT);
    for name in ["style.css", "image.png"] {
        let expected =
            url::Url::from_file_path(root.join(name)).map_err(|()| "invalid resource URL")?;
        assert!(
            requests
                .iter()
                .any(|row| row["url"].as_str() == Some(expected.as_str()))
        );
    }
    Ok(())
}

fn retain_runtime_state(session: &mut HtmlInteractiveSession) -> TestResult {
    let initial = session
        .runtime
        .observe_feature_state("window.resourceSentinel")?;
    assert_eq!(initial, INITIAL_SENTINEL);
    let text = session
        .runtime
        .observe_feature_state("document.getElementById('state').textContent")?;
    assert_eq!(text, "initial");
    let changed = session.runtime.observe_feature_state(
        "(window.resourceSentinel = 91, document.getElementById('state').textContent = 'retained', true)",
    )?;
    assert_eq!(changed, true);
    Ok(())
}

fn assert_fresh_session(
    root: &Path,
    source: HtmlBrowserSource,
    resized: HtmlBrowserViewport,
) -> TestResult {
    let fresh_audit = ResourceAuditScope::start();
    let fresh = HtmlInteractiveSession::start(source, resized)?;
    let fresh_inventory = audit(fresh_audit, root, "fresh-audit")?;
    assert!(!fresh_inventory.complete);
    assert!(fresh_inventory.reason.contains("image.png"));
    assert!(
        fresh_inventory
            .requests
            .as_array()
            .ok_or("missing requests")?
            .iter()
            .any(|row| row["url"]
                .as_str()
                .is_some_and(|url| url.ends_with("style.css")))
    );
    let frame = fresh.latest_frame().ok_or("missing fresh frame")?;
    assert_eq!(frame.viewport, resized);
    assert_color(frame, BLUE, true);
    assert_color(frame, GREEN, false);
    assert_color(frame, RED, false);
    Ok(())
}

#[test]
fn resize_reuses_loaded_resources_and_fresh_session_observes_changes() -> TestResult {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path();
    css(root, "#00a000")?;
    RgbaImage::from_pixel(16, 16, Rgba(RED)).save(root.join("image.png"))?;
    let source = source(root)?;
    let initial = HtmlBrowserViewport::new(100, 80, 1.0)?;
    let resized = HtmlBrowserViewport::new(120, 80, 1.0)?;

    let initial_audit = ResourceAuditScope::start();
    let mut session = HtmlInteractiveSession::start(source.clone(), initial)?;
    assert_initial_inventory(audit(initial_audit, root, "initial-audit")?, root)?;
    retain_runtime_state(&mut session)?;
    assert_frame_and_state(&mut session, initial)?;

    css(root, "#0000ff")?;
    std::fs::remove_file(root.join("image.png"))?;
    assert_retained_resources_after_resize_and_scroll(&mut session, root, initial, resized)?;
    assert_fresh_session(root, source, resized)
}

fn assert_retained_resources_after_resize_and_scroll(
    session: &mut HtmlInteractiveSession,
    root: &Path,
    initial: HtmlBrowserViewport,
    resized: HtmlBrowserViewport,
) -> TestResult {
    let retained_audit = ResourceAuditScope::start();
    session.resize(initial)?;
    assert_frame_and_state(session, initial)?;
    session.resize(resized)?;
    assert_frame_and_state(session, resized)?;
    scroll_retained_session(session, resized)?;
    let retained_inventory = audit(retained_audit, root, "resize-scroll-audit")?;
    assert!(retained_inventory.complete, "{}", retained_inventory.reason);
    assert_eq!(
        retained_inventory.requests.as_array().map(Vec::len),
        Some(0)
    );
    Ok(())
}
