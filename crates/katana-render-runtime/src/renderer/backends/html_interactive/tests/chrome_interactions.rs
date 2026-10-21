use super::super::HtmlInteractiveSession;
use super::support::{TestResult, click_element, to_string};
use crate::renderer::backends::html_browser::{
    HtmlBrowserInput, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::time::Instant;

const MILLIS_PER_SECOND: f64 = 1000.0;
const INTERACTION_CONTRACT_MAX_MS: u32 = 3000;

const INTERACTION_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/html/interaction-fidelity.html"
));

#[derive(Clone, Debug, PartialEq, Eq)]
struct SemanticState {
    expanded: Option<String>,
    input_value: Option<String>,
    result: String,
}

#[test]
fn interaction_state_is_derived_from_the_runtime_snapshot() -> TestResult {
    let action_state = action_state()?;
    assert_eq!(action_state.expanded.as_deref(), Some("true"));
    assert_eq!(action_state.result, "opened");

    let form_state = form_state()?;
    assert_eq!(form_state.input_value.as_deref(), Some("ok"));
    assert_eq!(form_state.result, "submitted");

    let link_url = link_url()?;
    assert!(link_url.contains("interaction-fidelity.html?linked=1"));

    let (scroll_y, content_height) = scroll_metrics()?;
    assert!(scroll_y > 0.0);
    write_report_if_requested(
        &link_url,
        &action_state,
        &form_state,
        scroll_y,
        content_height,
    )?;
    Ok(())
}

fn action_state() -> TestResult<SemanticState> {
    let mut session = session()?;
    click_element(&mut session, "action")?;
    state_from_snapshot(&session.runtime.snapshot().map_err(to_string)?)
}

fn form_state() -> TestResult<SemanticState> {
    let mut session = session()?;
    click_element(&mut session, "value")?;
    session
        .dispatch_input(HtmlBrowserInput::Text { text: "ok".into() })
        .map_err(to_string)?;
    click_element(&mut session, "submit")?;
    state_from_snapshot(&session.runtime.snapshot().map_err(to_string)?)
}

fn link_url() -> TestResult<String> {
    let mut session = session()?;
    click_element(&mut session, "link")?;
    session
        .take_navigation()
        .map(|event| event.url.as_str().to_string())
        .ok_or_else(|| "link navigation event is missing".to_string())
}

fn scroll_metrics() -> TestResult<(f32, f32)> {
    let mut session = session()?;
    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 600.0,
        })
        .map_err(to_string)?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "scroll frame is missing".to_string())?;
    Ok((frame.scroll_y, frame.content_height))
}

fn interaction_performance() -> TestResult<(f64, f64)> {
    let (source, viewport) = interaction_fixture()?;
    let started = Instant::now();
    let mut session = HtmlRuntime.open(source, viewport).map_err(to_string)?;
    let first_frame_ms = started.elapsed().as_secs_f64() * MILLIS_PER_SECOND;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "initial interaction performance frame is missing".to_string())?;
    if frame.pixels.is_empty() {
        return Err("initial interaction performance frame has no pixels".to_string());
    }
    let close_started = Instant::now();
    session.close().map_err(to_string)?;
    let close_ms = close_started.elapsed().as_secs_f64() * MILLIS_PER_SECOND;
    Ok((first_frame_ms, close_ms))
}

fn session() -> TestResult<HtmlInteractiveSession> {
    let (source, viewport) = interaction_fixture()?;
    HtmlInteractiveSession::start(source, viewport).map_err(to_string)
}

fn interaction_fixture() -> TestResult<(HtmlBrowserSource, HtmlBrowserViewport)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/html/interaction-fidelity.html")
        .canonicalize()
        .map_err(to_string)?;
    let origin =
        url::Url::from_file_path(path).map_err(|()| "invalid fixture origin".to_string())?;
    let source = HtmlBrowserSource::new(INTERACTION_FIXTURE, origin.as_str()).map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(1280, 900, 1.0).map_err(to_string)?;
    Ok((source, viewport))
}

fn state_from_snapshot(snapshot: &str) -> TestResult<SemanticState> {
    let expanded = attribute_value(snapshot, "aria-expanded");
    let input_value = attribute_value(snapshot, "value");
    let result = text_content(snapshot, "data-krr-result")
        .ok_or_else(|| "serialized interaction result is missing".to_string())?;
    Ok(SemanticState {
        expanded,
        input_value,
        result,
    })
}

fn attribute_value(snapshot: &str, attribute: &str) -> Option<String> {
    let marker = format!("{attribute}=\"");
    let start = snapshot.find(&marker)? + marker.len();
    let end = snapshot[start..].find('"')? + start;
    Some(snapshot[start..end].to_string())
}

fn text_content(snapshot: &str, attribute: &str) -> Option<String> {
    let marker = format!("{attribute}=\"");
    let start = snapshot.find(&marker)?;
    let tag_end = snapshot[start..].find('>')? + start + 1;
    let end = snapshot[tag_end..].find('<')? + tag_end;
    Some(snapshot[tag_end..end].to_string())
}

fn write_report_if_requested(
    link_url: &str,
    action_state: &SemanticState,
    form_state: &SemanticState,
    scroll_y: f32,
    content_height: f32,
) -> TestResult {
    let Some(report_path) = interaction_report_path() else {
        return Ok(());
    };
    let input_sha256 = interaction_fixture_sha256()?;
    let performance = interaction_performance()?;
    let report = interaction_report(
        input_sha256,
        link_url,
        action_state,
        form_state,
        scroll_y,
        content_height,
        performance,
    );
    write_interaction_report(report_path, &report)
}

fn interaction_report_path() -> Option<std::ffi::OsString> {
    std::env::var_os("KRR_HTML_INTERACTION_REPORT")
}

fn interaction_fixture_sha256() -> TestResult<String> {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/html/interaction-fidelity.html"
    );
    let bytes = fs::read(fixture).map_err(to_string)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(digest_hex(hasher.finalize().as_slice()))
}

fn write_interaction_report(
    report_path: std::ffi::OsString,
    report: &serde_json::Value,
) -> TestResult {
    fs::write(
        report_path,
        serde_json::to_vec_pretty(&report).map_err(to_string)?,
    )
    .map_err(to_string)
}

fn interaction_report(
    input_sha256: String,
    link_url: &str,
    action_state: &SemanticState,
    form_state: &SemanticState,
    scroll_y: f32,
    content_height: f32,
    performance: (f64, f64),
) -> serde_json::Value {
    let report = json!({
        "inputSha256": input_sha256,
        "browser": { "name": "KRR HtmlRuntime", "version": "test", "viewport": { "width": 1280, "height": 900 }, "scale": 1 },
        "performance": { "firstFrameMs": performance.0, "closeMs": performance.1, "contractMaxMs": INTERACTION_CONTRACT_MAX_MS },
        "states": {
            "link": { "url": link_url },
            "scroll": { "scrollY": scroll_y, "contentHeight": content_height, "viewportHeight": 900 },
            "form": { "value": form_state.input_value, "result": form_state.result },
            "js": { "expanded": action_state.expanded.as_deref().map(|value| value == "true"), "result": action_state.result },
        }
    });
    report
}

fn digest_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
