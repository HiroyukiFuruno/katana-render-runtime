use super::CatalogOperations;
use crate::options::{CatalogInput, CatalogOperationPlan};
use crate::{phase_snapshot, HtmlBrowserSession, SessionFrame};
#[cfg(feature = "baseline")]
use baseline::HtmlBrowserInput;
#[cfg(feature = "candidate")]
use candidate::HtmlBrowserInput;

const MAX_CATALOG_TIME_ADVANCE_MS: f64 = 60_000.0;
#[cfg(feature = "candidate")]
const MILLISECONDS_PER_SECOND: f64 = 1_000.0;

pub(super) fn perform(
    session: &mut HtmlBrowserSession,
    plan: &CatalogOperationPlan,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    validate_catalog_dispatch_plan(plan)?;
    let operation_start = std::time::Instant::now();
    let before = session.frame_state()?;
    let mut dispatched = Vec::with_capacity(plan.operation_stream.len());
    for input in &plan.operation_stream {
        dispatched.push(dispatch_catalog_input(session, input)?);
    }
    let after = session.frame_state()?;
    Ok(serde_json::json!({
        "measurement": phase_snapshot(operation_start, session)?,
        "catalog_operation": {
            "condition_id": plan.condition_id,
            "case_id": plan.case_id,
            "state": plan.state,
            "operation_kind": plan.operation_kind,
            "operation_evidence_sha256": plan.operation_evidence_sha256,
            "dispatched_inputs": dispatched,
            "frame_before": before,
            "frame_after": after,
        }
    }))
}

fn validate_catalog_dispatch_plan(
    plan: &CatalogOperationPlan,
) -> Result<(), Box<dyn std::error::Error>> {
    if CatalogOperations::is_static(plan) {
        if !plan.operation_stream.is_empty() {
            return Err("static catalog operation must not dispatch inputs".into());
        }
        return Err("static catalog operation requires a measured close/reopen".into());
    }
    if plan.operation_stream.is_empty() {
        return Err("catalog operation has no observed input stream".into());
    }
    Ok(())
}

enum CatalogDispatch {
    BrowserInput(HtmlBrowserInput, &'static str),
    PointerButton(f32, f32, u8, bool),
    AdvanceTime(f64),
}

fn dispatch_catalog_input(
    session: &mut HtmlBrowserSession,
    input: &CatalogInput,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    match classify_catalog_input(input) {
        CatalogDispatch::BrowserInput(input, name) => dispatch_browser_input(session, input, name),
        CatalogDispatch::PointerButton(x, y, button, is_down) => {
            dispatch_pointer_button(session, x, y, button, is_down)
        }
        CatalogDispatch::AdvanceTime(elapsed_ms) => dispatch_time_advance(session, elapsed_ms),
    }
}

fn classify_catalog_input(input: &CatalogInput) -> CatalogDispatch {
    match input {
        CatalogInput::PointerMove { x, y } => CatalogDispatch::BrowserInput(
            HtmlBrowserInput::PointerMove { x: *x, y: *y },
            "pointer_move",
        ),
        CatalogInput::PointerDown { x, y, button } | CatalogInput::PointerUp { x, y, button } => {
            CatalogDispatch::PointerButton(
                *x,
                *y,
                *button,
                matches!(input, CatalogInput::PointerDown { .. }),
            )
        }
        CatalogInput::Scroll { delta_x, delta_y } => CatalogDispatch::BrowserInput(
            HtmlBrowserInput::Scroll {
                delta_x: *delta_x,
                delta_y: *delta_y,
            },
            "scroll",
        ),
        CatalogInput::Text { text } => {
            CatalogDispatch::BrowserInput(HtmlBrowserInput::Text { text: text.clone() }, "text")
        }
        CatalogInput::KeyDown { key } | CatalogInput::KeyUp { key } => {
            catalog_key_input(key, matches!(input, CatalogInput::KeyDown { .. }))
        }
        CatalogInput::AdvanceTime { elapsed_ms } => CatalogDispatch::AdvanceTime(*elapsed_ms),
    }
}

fn catalog_key_input(key: &str, is_down: bool) -> CatalogDispatch {
    let (input, name) = if is_down {
        (
            HtmlBrowserInput::KeyDown {
                key: key.to_owned(),
            },
            "key_down",
        )
    } else {
        (
            HtmlBrowserInput::KeyUp {
                key: key.to_owned(),
            },
            "key_up",
        )
    };
    CatalogDispatch::BrowserInput(input, name)
}

fn dispatch_browser_input(
    session: &mut HtmlBrowserSession,
    input: HtmlBrowserInput,
    name: &'static str,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    session.dispatch_input(input)?;
    Ok(name)
}

fn dispatch_pointer_button(
    session: &mut HtmlBrowserSession,
    x: f32,
    y: f32,
    button: u8,
    is_down: bool,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    let (input, name) = if is_down {
        (
            HtmlBrowserInput::PointerDown { x, y, button },
            "pointer_down",
        )
    } else {
        (HtmlBrowserInput::PointerUp { x, y, button }, "pointer_up")
    };
    dispatch_browser_input(session, input, name)
}

fn dispatch_time_advance(
    session: &mut HtmlBrowserSession,
    elapsed_ms: f64,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    if !elapsed_ms.is_finite() || !(0.0..=MAX_CATALOG_TIME_ADVANCE_MS).contains(&elapsed_ms) {
        return Err("catalog time advance must be within the supported finite range".into());
    }
    dispatch_supported_time_advance(session, elapsed_ms)
}

#[cfg(feature = "candidate")]
fn dispatch_supported_time_advance(
    session: &mut HtmlBrowserSession,
    elapsed_ms: f64,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    session.advance_time(std::time::Duration::from_secs_f64(
        elapsed_ms / MILLISECONDS_PER_SECOND,
    ))?;
    Ok("advance_time")
}

#[cfg(feature = "baseline")]
fn dispatch_supported_time_advance(
    _session: &mut HtmlBrowserSession,
    _elapsed_ms: f64,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    Err("baseline does not support catalog time advance".into())
}
