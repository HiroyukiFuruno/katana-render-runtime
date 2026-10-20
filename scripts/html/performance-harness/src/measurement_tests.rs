use super::{measure_post_operation, HtmlBrowserSession, HtmlBrowserViewport};
use crate::options::{CatalogInput, CatalogOperationPlan, OperationPlan};
use crate::test_support::TestSupport;
#[cfg(feature = "baseline")]
use baseline::{HtmlBrowserSource, HtmlRuntime};
#[cfg(feature = "candidate")]
use candidate::{HtmlBrowserSource, HtmlRuntime};

const VIEWPORT: (u32, u32, f32) = (1280, 900, 1.0);

fn open_fixture(
    markup: &str,
    origin: &str,
) -> Result<HtmlBrowserSession, Box<dyn std::error::Error>> {
    let viewport = HtmlBrowserViewport::new(VIEWPORT.0, VIEWPORT.1, VIEWPORT.2)?;
    Ok(HtmlRuntime.open(HtmlBrowserSource::new(markup.to_owned(), origin)?, viewport)?)
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
