#![cfg(test)]

use super::{measure_full_y, prepare_top_scroll, scroll_to_bottom, set_max_frame_elapsed_ms};
use crate::{options::LegacyOperationPlan, HtmlBrowserSession, HtmlBrowserViewport, PhaseSnapshot};
#[cfg(feature = "baseline")]
use baseline::{HtmlBrowserInput, HtmlBrowserSource, HtmlRuntime};
#[cfg(feature = "candidate")]
use candidate::{HtmlBrowserInput, HtmlBrowserSource, HtmlRuntime};

const TALL_PAGE_VIEWPORT_WIDTH: u32 = 160;
const TALL_PAGE_VIEWPORT_HEIGHT: u32 = 120;
const NOOP_POINTER_X: f32 = 1_000.0;
const NOOP_POINTER_Y: f32 = 1_000.0;
const SCROLL_TO_NONZERO_Y: f32 = 60.0;
const SHORT_DOCUMENT_MISMATCH_VIEWPORT_HEIGHT: u32 = 500;
const BOTTOM_CLAMP_MISMATCH_VIEWPORT_HEIGHT: u32 = 220;
const MAX_FRAME_ELAPSED_MS: f64 = 1.0;

fn open_tall_session() -> Result<HtmlBrowserSession, Box<dyn std::error::Error>> {
    let viewport =
        HtmlBrowserViewport::new(TALL_PAGE_VIEWPORT_WIDTH, TALL_PAGE_VIEWPORT_HEIGHT, 1.0)?;
    let html = "<style>body{margin:0}</style><div style='height:400px'></div>";
    let source = HtmlBrowserSource::new(html.to_owned(), "https://example.test/full-y.html")?;
    Ok(HtmlRuntime.open(source, viewport)?)
}

fn dispatch_scroll(
    session: &mut HtmlBrowserSession,
    delta_y: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    session.dispatch_input(HtmlBrowserInput::Scroll {
        delta_x: 0.0,
        delta_y,
    })?;
    Ok(())
}

#[test]
fn full_y_records_bottom_frame_and_returns_real_session_to_top(
) -> Result<(), Box<dyn std::error::Error>> {
    for height in [80, 1800] {
        let viewport =
            HtmlBrowserViewport::new(TALL_PAGE_VIEWPORT_WIDTH, TALL_PAGE_VIEWPORT_HEIGHT, 1.0)?;
        let html = format!(
            "<style>body{{margin:0}}</style><div style='height:{height}px;background:#abcdef'></div>"
        );
        let source = HtmlBrowserSource::new(html, "https://example.test/full-y.html")?;
        let mut session = HtmlRuntime.open(source, viewport)?;
        let (measurement, steps, final_y, max_scroll) = measure_full_y(&mut session, viewport)?;
        let measurement = serde_json::Value::Object(measurement);
        assert_eq!(steps > 0, max_scroll > 0.0);
        assert_eq!(final_y, max_scroll);
        assert_eq!(
            measurement["frame"]["scroll_y"].as_f64(),
            Some(f64::from(max_scroll))
        );
        assert_full_y_measurement(&measurement, &session, max_scroll)?;
        session.close()?;
    }
    Ok(())
}

fn assert_full_y_measurement(
    measurement: &serde_json::Value,
    session: &HtmlBrowserSession,
    max_scroll: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_full_y_measurement_shape(measurement)?;
    assert_full_y_measurement_timing(measurement)?;
    assert_full_y_restored_frame(measurement, session, max_scroll)?;
    Ok(())
}

fn assert_full_y_measurement_shape(
    measurement: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let object = measurement
        .as_object()
        .ok_or("full-Y snapshot must serialize as an object")?;
    let mut keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "elapsed_ms",
            "frame",
            "max_frame_elapsed_ms",
            "peak_rss_bytes",
            "peak_rss_scope",
        ]
        .to_vec()
    );
    assert_eq!(
        measurement["peak_rss_scope"],
        "process-cumulative-including-phase-setup"
    );
    Ok(())
}

fn assert_full_y_measurement_timing(
    measurement: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let elapsed = measurement["elapsed_ms"]
        .as_f64()
        .ok_or("missing elapsed")?;
    let maximum = measurement["max_frame_elapsed_ms"]
        .as_f64()
        .ok_or("missing maximum frame time")?;
    assert!(elapsed.is_finite() && maximum.is_finite());
    assert!(maximum >= 0.0 && elapsed >= maximum);
    assert!(
        measurement["peak_rss_bytes"]
            .as_u64()
            .ok_or("missing RSS")?
            > 0
    );
    Ok(())
}

fn assert_full_y_restored_frame(
    measurement: &serde_json::Value,
    session: &HtmlBrowserSession,
    max_scroll: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let top_frame = session.latest_frame().ok_or("missing restored top frame")?;
    let bottom_generation = measurement["frame"]["generation"]
        .as_u64()
        .ok_or("missing captured bottom generation")?;
    assert_eq!(top_frame.scroll_y, 0.0);
    assert_eq!(top_frame.generation > bottom_generation, max_scroll > 0.0);
    Ok(())
}

#[test]
fn full_y_helpers_reject_missing_progress_and_incomplete_top_restore(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = open_tall_session()?;
    let mut scroll_y = 0.0;
    let mut steps = 0;
    let stalled = scroll_to_bottom(&mut session, &mut scroll_y, 10_000.0, &mut steps);
    assert_eq!(
        stalled
            .err()
            .ok_or("scroll unexpectedly reached requested extent")?
            .to_string(),
        "full-Y scroll made no progress before bottom"
    );
    assert!(steps > 0 && scroll_y > 0.0);
    session.close()?;
    let mut session = open_tall_session()?;
    dispatch_scroll(&mut session, 1_000.0)?;
    let actual_scroll = session
        .latest_frame()
        .ok_or("missing scrolled frame")?
        .scroll_y;
    assert!(actual_scroll > 1.0);
    assert_eq!(
        prepare_top_scroll(&mut session, actual_scroll / 2.0)
            .err()
            .ok_or("undersized restore delta reached top")?
            .to_string(),
        "full-Y preparation did not return to scroll origin"
    );
    session.close()?;
    Ok(())
}

#[test]
fn full_y_scroll_helpers_propagate_errors_from_a_closed_real_session(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = open_tall_session()?;
    session.close()?;

    let mut scroll_y = 0.0;
    let mut steps = 0;
    assert!(scroll_to_bottom(&mut session, &mut scroll_y, 1.0, &mut steps).is_err());
    assert!(prepare_top_scroll(&mut session, 1.0).is_err());
    Ok(())
}

#[test]
fn pointer_operation_rejects_a_real_noop_outside_the_viewport(
) -> Result<(), Box<dyn std::error::Error>> {
    let viewport =
        HtmlBrowserViewport::new(TALL_PAGE_VIEWPORT_WIDTH, TALL_PAGE_VIEWPORT_HEIGHT, 1.0)?;
    let source = HtmlBrowserSource::new(
        "<style>body{margin:0}</style><div style='height:400px'></div>".to_owned(),
        "https://example.test/noop-pointer.html",
    )?;
    let mut session = HtmlRuntime.open(source, viewport)?;
    let plan = LegacyOperationPlan {
        fixture_sha256: String::new(),
        operation: "outside-viewport-noop".to_owned(),
        x: NOOP_POINTER_X,
        y: NOOP_POINTER_Y,
        expected_dom_state_path: std::path::PathBuf::new(),
        expected_dom_state_sha256: String::new(),
    };

    assert_eq!(
        super::perform_operation(&mut session, &plan)
            .err()
            .ok_or("no-op pointer operation unexpectedly succeeded")?
            .to_string(),
        "post-operation did not advance frame generation"
    );
    session.close()?;
    Ok(())
}

#[test]
fn full_y_rejects_a_short_document_measurement_from_a_scrolled_real_session(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = open_tall_session()?;
    dispatch_scroll(&mut session, SCROLL_TO_NONZERO_Y)?;
    let oversized_viewport = HtmlBrowserViewport::new(
        TALL_PAGE_VIEWPORT_WIDTH,
        SHORT_DOCUMENT_MISMATCH_VIEWPORT_HEIGHT,
        1.0,
    )?;

    assert_eq!(
        measure_full_y(&mut session, oversized_viewport)
            .err()
            .ok_or("short-document mismatch unexpectedly succeeded")?
            .to_string(),
        "short-document full-Y capture must begin at the top"
    );
    session.close()?;
    Ok(())
}

#[test]
fn full_y_rejects_a_bottom_clamp_mismatch_with_a_real_session(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = open_tall_session()?;
    let undersized_viewport = HtmlBrowserViewport::new(
        TALL_PAGE_VIEWPORT_WIDTH,
        BOTTOM_CLAMP_MISMATCH_VIEWPORT_HEIGHT,
        1.0,
    )?;

    assert_eq!(
        measure_full_y(&mut session, undersized_viewport)
            .err()
            .ok_or("bottom-clamp mismatch unexpectedly succeeded")?
            .to_string(),
        "full-Y did not reach the actual bottom clamp"
    );
    session.close()?;
    Ok(())
}

#[test]
fn max_frame_time_is_inserted_into_an_object_snapshot() {
    let mut measurement = PhaseSnapshot::new();
    set_max_frame_elapsed_ms(&mut measurement, MAX_FRAME_ELAPSED_MS);
    assert_eq!(
        serde_json::Value::Object(measurement),
        serde_json::json!({"max_frame_elapsed_ms": MAX_FRAME_ELAPSED_MS})
    );
}
