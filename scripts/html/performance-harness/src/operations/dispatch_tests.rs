use super::{dispatch_catalog_input, perform, validate_catalog_dispatch_plan};
use crate::options::{CatalogInput, CatalogOperationPlan, CatalogViewport};
use crate::{HtmlBrowserSession, HtmlBrowserViewport};
#[cfg(feature = "baseline")]
use baseline::{HtmlBrowserSource, HtmlRuntime};
#[cfg(feature = "candidate")]
use candidate::{HtmlBrowserSource, HtmlRuntime};

const VIEWPORT_WIDTH: u32 = 320;
const VIEWPORT_HEIGHT: u32 = 240;
const DEVICE_SCALE_FACTOR: f32 = 1.0;
const INPUT_X: f32 = 80.0;
const INPUT_Y: f32 = 160.0;
const SUBMIT_Y: f32 = 220.0;
const POINTER_BUTTON: u8 = 0;
const SCROLL_DELTA_X: f32 = 0.0;
const SCROLL_DELTA_Y: f32 = 120.0;
const TIMER_FIXTURE_URL: &str = "https://example.test/script-async.html";
const VALID_TIME_ADVANCES_MS: [f64; 2] = [0.0, super::MAX_CATALOG_TIME_ADVANCE_MS];
const INVALID_TIME_ADVANCES_MS: [f64; 2] = [-1.0, super::MAX_CATALOG_TIME_ADVANCE_MS + 1.0];

fn plan(operation_kind: &str, operation_stream: Vec<CatalogInput>) -> CatalogOperationPlan {
    CatalogOperationPlan {
        profile: "dispatch-unit".to_owned(),
        condition_id: "dispatch-unit/expected-state".to_owned(),
        case_id: "dispatch-unit".to_owned(),
        state: "expected-state".to_owned(),
        operation_kind: operation_kind.to_owned(),
        target_selector: None,
        fixture_sha256: String::new(),
        viewport: CatalogViewport {
            width: VIEWPORT_WIDTH,
            height: VIEWPORT_HEIGHT,
            scale: DEVICE_SCALE_FACTOR,
        },
        operation_evidence_path: std::path::PathBuf::new(),
        operation_evidence_sha256: String::new(),
        evidence_binary_sha256: String::new(),
        operation_stream,
    }
}

fn open_interaction_session() -> Result<HtmlBrowserSession, Box<dyn std::error::Error>> {
    open_session(
        include_str!("../../../../../tests/fixtures/html/interaction-fidelity.html"),
        "https://example.test/interaction-fidelity.html",
    )
}

fn open_timer_session() -> Result<HtmlBrowserSession, Box<dyn std::error::Error>> {
    open_session(
        include_str!("../../../../../tests/fixtures/html/aspects/script-async.html"),
        TIMER_FIXTURE_URL,
    )
}

fn open_session(markup: &str, url: &str) -> Result<HtmlBrowserSession, Box<dyn std::error::Error>> {
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, DEVICE_SCALE_FACTOR)?;
    let source = HtmlBrowserSource::new(markup.to_owned(), url)?;
    Ok(HtmlRuntime.open(source, viewport)?)
}

fn frame_generation(session: &HtmlBrowserSession) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(session
        .latest_frame()
        .ok_or("dispatch test session has no frame")?
        .generation)
}

fn frame_pixels(session: &HtmlBrowserSession) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(session
        .latest_frame()
        .ok_or("dispatch test session has no frame")?
        .pixels
        .clone())
}

fn dispatch_error(
    session: &mut HtmlBrowserSession,
    operation_plan: &CatalogOperationPlan,
) -> Result<String, Box<dyn std::error::Error>> {
    match perform(session, operation_plan) {
        Err(error) => Ok(error.to_string()),
        Ok(_) => Err("catalog dispatch unexpectedly succeeded".into()),
    }
}

fn assert_dispatch_error(
    operation_kind: &str,
    operation_stream: Vec<CatalogInput>,
    expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let operation_plan = plan(operation_kind, operation_stream);
    let error = validate_catalog_dispatch_plan(&operation_plan)
        .err()
        .ok_or("invalid catalog dispatch plan was accepted")?;
    assert_eq!(error.to_string(), expected);
    Ok(())
}

#[test]
fn dispatches_real_event_order_and_scroll_in_an_actual_session(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = open_interaction_session()?;
    let before_generation = frame_generation(&session)?;
    let before_pixels = frame_pixels(&session)?;
    let operation_plan = plan("script-real-events", real_event_stream());
    let result = perform(&mut session, &operation_plan)?;
    assert_dispatched_event_order(&result)?;
    assert!(frame_generation(&session)? > before_generation);
    assert_ne!(frame_pixels(&session)?, before_pixels);
    assert_scroll_dispatch(&mut session)?;
    session.close()?;
    Ok(())
}

fn real_event_stream() -> Vec<CatalogInput> {
    let mut inputs = click_stream(INPUT_X, INPUT_Y);
    inputs.extend([
        CatalogInput::Text {
            text: "入力".to_owned(),
        },
        CatalogInput::KeyDown {
            key: "Shift".to_owned(),
        },
        CatalogInput::KeyUp {
            key: "Shift".to_owned(),
        },
    ]);
    inputs.extend(click_stream(INPUT_X, SUBMIT_Y));
    inputs
}

fn click_stream(x: f32, y: f32) -> Vec<CatalogInput> {
    vec![
        CatalogInput::PointerMove { x, y },
        CatalogInput::PointerDown {
            x,
            y,
            button: POINTER_BUTTON,
        },
        CatalogInput::PointerUp {
            x,
            y,
            button: POINTER_BUTTON,
        },
    ]
}

fn assert_dispatched_event_order(
    result: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        result["catalog_operation"]["dispatched_inputs"],
        serde_json::json!([
            "pointer_move",
            "pointer_down",
            "pointer_up",
            "text",
            "key_down",
            "key_up",
            "pointer_move",
            "pointer_down",
            "pointer_up"
        ])
    );
    Ok(())
}

fn assert_scroll_dispatch(
    session: &mut HtmlBrowserSession,
) -> Result<(), Box<dyn std::error::Error>> {
    let before = session
        .latest_frame()
        .ok_or("scroll test session has no frame")?
        .scroll_y;
    let input = CatalogInput::Scroll {
        delta_x: SCROLL_DELTA_X,
        delta_y: SCROLL_DELTA_Y,
    };
    assert_eq!(dispatch_catalog_input(session, &input)?, "scroll");
    let after = session
        .latest_frame()
        .ok_or("scroll dispatch did not produce a frame")?
        .scroll_y;
    assert!(after > before);
    Ok(())
}

#[test]
fn rejects_static_and_empty_streams_with_their_existing_errors(
) -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "script-await-mutation-delivery",
            Vec::new(),
            "static catalog operation requires a measured close/reopen",
        ),
        (
            "input-state-empty",
            vec![CatalogInput::PointerMove {
                x: INPUT_X,
                y: INPUT_Y,
            }],
            "static catalog operation must not dispatch inputs",
        ),
        (
            "script-real-events",
            Vec::new(),
            "catalog operation has no observed input stream",
        ),
    ];
    for (kind, stream, expected) in cases {
        assert_dispatch_error(kind, stream, expected)?;
    }
    Ok(())
}

#[test]
fn applies_time_advance_bounds_and_arm_support_with_a_real_session(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = open_interaction_session()?;
    for elapsed_ms in INVALID_TIME_ADVANCES_MS {
        let operation_plan = plan(
            "script-await-timer-microtask",
            vec![CatalogInput::AdvanceTime { elapsed_ms }],
        );
        assert_eq!(
            dispatch_error(&mut session, &operation_plan)?,
            "catalog time advance must be within the supported finite range"
        );
    }
    session.close()?;
    assert_arm_time_advance()?;
    Ok(())
}

#[cfg(feature = "candidate")]
fn assert_arm_time_advance() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = open_timer_session()?;
    let before_generation = frame_generation(&session)?;
    let before_pixels = frame_pixels(&session)?;
    for elapsed_ms in VALID_TIME_ADVANCES_MS {
        let result = perform(&mut session, &time_advance_plan(elapsed_ms))?;
        assert_time_advance_dispatched(&result)?;
        assert_eq!(frame_generation(&session)?, before_generation);
        assert_eq!(frame_pixels(&session)?, before_pixels);
    }
    session.close()?;
    Ok(())
}

#[cfg(feature = "baseline")]
fn assert_arm_time_advance() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = open_timer_session()?;
    assert_eq!(
        dispatch_error(&mut session, &time_advance_plan(VALID_TIME_ADVANCES_MS[0]))?,
        "baseline does not support catalog time advance"
    );
    session.close()?;
    Ok(())
}

fn time_advance_plan(elapsed_ms: f64) -> CatalogOperationPlan {
    plan(
        "script-await-timer-microtask",
        vec![CatalogInput::AdvanceTime { elapsed_ms }],
    )
}

#[cfg(feature = "candidate")]
fn assert_time_advance_dispatched(
    result: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        result["catalog_operation"]["dispatched_inputs"],
        serde_json::json!(["advance_time"])
    );
    Ok(())
}

#[cfg(feature = "candidate")]
#[test]
fn closed_session_time_advance_returns_session_closed_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = open_timer_session()?;
    session.close()?;
    let error = dispatch_catalog_input(
        &mut session,
        &CatalogInput::AdvanceTime {
            elapsed_ms: VALID_TIME_ADVANCES_MS[0],
        },
    )
    .err()
    .ok_or("closed session accepted time advance")?;
    assert_eq!(error.to_string(), "browser session is closed");
    Ok(())
}
