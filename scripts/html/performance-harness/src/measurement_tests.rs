#![cfg(test)]

use super::{measure_post_operation, HtmlBrowserSession, HtmlBrowserViewport};
use crate::options::{CatalogInput, CatalogOperationPlan, OperationPlan};
use crate::test_support::TestSupport;
#[cfg(feature = "baseline")]
use baseline::{HtmlBrowserSource, HtmlRuntime};
#[cfg(feature = "candidate")]
use candidate::{HtmlBrowserSource, HtmlRuntime};

#[path = "measurement_legacy_tests.rs"]
mod legacy_tests;

const VIEWPORT: (u32, u32, f32) = (1280, 900, 1.0);

fn open_fixture(
    markup: &str,
    origin: &str,
) -> Result<HtmlBrowserSession, Box<dyn std::error::Error>> {
    open_fixture_at(
        markup,
        origin,
        HtmlBrowserViewport::new(VIEWPORT.0, VIEWPORT.1, VIEWPORT.2)?,
    )
}

fn open_fixture_at(
    markup: &str,
    origin: &str,
    viewport: HtmlBrowserViewport,
) -> Result<HtmlBrowserSession, Box<dyn std::error::Error>> {
    Ok(HtmlRuntime.open(HtmlBrowserSource::new(markup.to_owned(), origin)?, viewport)?)
}

fn assert_error_message<T>(
    result: Result<T, Box<dyn std::error::Error>>,
    expected: &str,
    accepted_message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = result.err().ok_or(accepted_message)?.to_string();
    assert_eq!(actual, expected);
    Ok(())
}

fn catalog_plan(
    operation_kind: &str,
    operation_stream: Vec<CatalogInput>,
) -> Result<CatalogOperationPlan, Box<dyn std::error::Error>> {
    let mut plan: CatalogOperationPlan = serde_json::from_value(TestSupport::catalog_test_plan())?;
    plan.operation_kind = operation_kind.to_owned();
    plan.operation_stream = operation_stream;
    Ok(plan)
}

fn assert_reopen_result(
    result: &serde_json::Value,
    operation_kind: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        result["catalog_operation"]["operation_kind"],
        operation_kind
    );
    assert_eq!(
        result["catalog_operation"]["delivery"],
        "measured-close-and-new-session-open"
    );
    let operation = &result["catalog_operation"];
    assert_eq!(operation["condition_id"], "async/expected-state/1280x900@1");
    assert_eq!(operation["case_id"], "async");
    assert_eq!(operation["state"], "expected-state");
    assert_eq!(operation["operation_evidence_sha256"], "b".repeat(64));
    assert_reopen_frame(&operation["frame_after"])
}

fn assert_reopen_frame(frame: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(frame["viewport"]["width"], VIEWPORT.0);
    assert_eq!(frame["viewport"]["height"], VIEWPORT.1);
    assert!(frame["generation"].as_u64().ok_or("missing generation")? > 0);
    assert!(frame["pixel_bytes"].as_u64().ok_or("missing pixels")? > 0);
    Ok(())
}

struct ReopenCase {
    html: &'static str,
    origin: &'static str,
    operation_kind: &'static str,
    settled_load: bool,
}

fn run_reopen_case(case: ReopenCase) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = open_fixture(case.html, case.origin)?;
    let plan = catalog_plan(case.operation_kind, Vec::new())?;
    let result = measure_post_operation(
        &mut session,
        &OperationPlan::Catalog(plan),
        case.html.as_bytes(),
        case.origin,
        HtmlBrowserViewport::new(VIEWPORT.0, VIEWPORT.1, VIEWPORT.2)?,
        case.settled_load,
    )?;
    assert_reopen_result(&result, case.operation_kind)?;
    session.close()?;
    Ok(())
}

#[test]
fn catalog_measurement_reopens_static_and_settled_sessions(
) -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        ReopenCase {
            html: include_str!("../../../../tests/fixtures/html/aspects/script-observers.html"),
            origin: "https://example.test/script-observers.html",
            operation_kind: "script-load-observe",
            settled_load: false,
        },
        ReopenCase {
            html: include_str!("../../../../tests/fixtures/html/aspects/script-async.html"),
            origin: "https://example.test/script-async.html",
            operation_kind: "script-await-timer-microtask",
            settled_load: true,
        },
    ];
    for case in cases {
        run_reopen_case(case)?;
    }
    Ok(())
}

struct ScrollCase {
    result: serde_json::Value,
    before_generation: u64,
    session: HtmlBrowserSession,
}

fn run_scroll_case() -> Result<ScrollCase, Box<dyn std::error::Error>> {
    let html = include_str!("../../../../tests/fixtures/html/interaction-fidelity.html");
    let origin = "https://example.test/interaction-fidelity.html";
    let mut session = open_fixture(html, origin)?;
    let before = session
        .latest_frame()
        .ok_or("missing initial frame")?
        .generation;
    let plan = catalog_plan(
        "script-real-events",
        vec![CatalogInput::Scroll {
            delta_x: 0.0,
            delta_y: 120.0,
        }],
    )?;
    let result = measure_post_operation(
        &mut session,
        &OperationPlan::Catalog(plan),
        html.as_bytes(),
        origin,
        HtmlBrowserViewport::new(VIEWPORT.0, VIEWPORT.1, VIEWPORT.2)?,
        false,
    )?;
    Ok(ScrollCase {
        result,
        before_generation: before,
        session,
    })
}

#[test]
fn stream_catalog_measurement_dispatches_real_scroll() -> Result<(), Box<dyn std::error::Error>> {
    let ScrollCase {
        result,
        before_generation,
        session,
    } = run_scroll_case()?;
    assert_eq!(
        result["catalog_operation"]["dispatched_inputs"],
        serde_json::json!(["scroll"])
    );
    let frame = session.latest_frame().ok_or("missing scroll frame")?;
    assert!(frame.generation > before_generation && frame.scroll_y > 0.0);
    assert!(!frame.pixels.is_empty());
    let mut session = session;
    session.close()?;
    Ok(())
}

fn assert_real_session_resize(
    width: u32,
    height: u32,
    scale: f32,
    expected_width: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let viewport = HtmlBrowserViewport::new(width, height, scale)?;
    let mut session = open_fixture_at(
        "<html><body>x</body></html>",
        "https://example.test/resize",
        viewport,
    )?;
    let result = super::measure_same_session_resize(&mut session, viewport)?;
    let evidence = &result["resize_evidence"];
    let frame = session.latest_frame().ok_or("missing resized frame")?;
    assert_eq!(evidence["to_viewport"]["width"], expected_width);
    assert_eq!(frame.viewport.width, expected_width);
    assert_eq!(frame.viewport.height, height);
    assert!((frame.viewport.device_scale_factor - scale).abs() < f32::EPSILON);
    let generation_before = evidence["generation_before"]
        .as_u64()
        .ok_or("missing pre-resize generation")?;
    let generation_after = evidence["generation_after"]
        .as_u64()
        .ok_or("missing post-resize generation")?;
    assert!(generation_after > generation_before);
    assert_eq!(evidence["real_raster_frame"], true);
    assert!(!frame.pixels.is_empty());
    session.close()?;
    Ok(())
}

#[test]
fn real_session_resize_handles_minimum_regular_and_scaled_viewports(
) -> Result<(), Box<dyn std::error::Error>> {
    for (width, height, scale, expected_width) in
        [(1, 1, 1.0, 2), (1280, 900, 1.0, 960), (1280, 900, 2.0, 960)]
    {
        assert_real_session_resize(width, height, scale, expected_width)?;
    }
    Ok(())
}

#[test]
fn resize_viewport_rejects_invalid_height_and_scale_in_both_width_paths(
) -> Result<(), Box<dyn std::error::Error>> {
    for (width, height, scale) in [(1, 0, 1.0), (2, 0, 1.0), (1, 1, 0.0), (2, 1, 0.0)] {
        let mut viewport = HtmlBrowserViewport::new(2, 1, 1.0)?;
        viewport.width = width;
        viewport.height = height;
        viewport.device_scale_factor = scale;
        assert!(super::narrower_resize_viewport(viewport).is_err());
    }
    Ok(())
}

#[test]
fn resized_frame_validator_rejects_stale_generation_and_wrong_viewport(
) -> Result<(), Box<dyn std::error::Error>> {
    let viewport = HtmlBrowserViewport::new(1, 1, 1.0)?;
    let session = open_fixture_at(
        "<html><body>x</body></html>",
        "https://example.test/resize",
        viewport,
    )?;
    let frame = session.latest_frame().ok_or("missing initial frame")?;
    let stale = super::validate_resized_frame(&session, viewport, frame.generation);
    assert_error_message(
        stale,
        "resize did not advance frame generation",
        "stale generation was accepted",
    )?;
    let wrong_viewport = HtmlBrowserViewport::new(2, 1, 1.0)?;
    let mismatch = super::validate_resized_frame(&session, wrong_viewport, 0);
    assert_error_message(
        mismatch,
        "resize produced an invalid raster frame",
        "wrong viewport was accepted",
    )?;
    let mut session = session;
    session.close()?;
    Ok(())
}
