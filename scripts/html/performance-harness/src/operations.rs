use std::time::Instant;

use super::options::OperationPlan;
use super::{hex_digest, phase_snapshot, HtmlBrowserSession, HtmlBrowserViewport};
#[cfg(feature = "baseline")]
use baseline::HtmlBrowserInput;
#[cfg(feature = "candidate")]
use candidate::HtmlBrowserInput;

const SCROLL_STEP: f32 = 600.0;
const POINTER_EVENT_SEQUENCE_LENGTH: usize = 3;

pub(super) type OperationMeasurement = (serde_json::Value, u64, u64, String, String);

pub(super) fn perform_operation(
    session: &mut HtmlBrowserSession,
    operation_plan: &OperationPlan,
) -> Result<OperationMeasurement, Box<dyn std::error::Error>> {
    let before_frame = session
        .latest_frame()
        .ok_or("post-operation requires a prepared frame")?;
    let previous_generation = before_frame.generation;
    let before_pixels = hex_digest(&before_frame.pixels);
    let post_operation = measure_pointer_action(session, operation_plan)?;
    let after_frame = session
        .latest_frame()
        .ok_or("post-operation did not produce a frame")?;
    let operation_generation = after_frame.generation;
    if operation_generation <= previous_generation {
        return Err("post-operation did not advance frame generation".into());
    }
    let after_pixels = hex_digest(&after_frame.pixels);
    Ok((
        post_operation,
        previous_generation,
        operation_generation,
        before_pixels,
        after_pixels,
    ))
}

fn measure_pointer_action(
    session: &mut HtmlBrowserSession,
    operation_plan: &OperationPlan,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let operation_start = Instant::now();
    for input in pointer_click_inputs(operation_plan.x, operation_plan.y) {
        session.dispatch_input(input)?;
    }
    phase_snapshot(operation_start, session)
}

pub(super) type FullYMeasurement = (serde_json::Value, usize, f32, f32);

pub(super) fn measure_full_y(
    session: &mut HtmlBrowserSession,
    viewport: HtmlBrowserViewport,
) -> Result<FullYMeasurement, Box<dyn std::error::Error>> {
    let full_y_start = Instant::now();
    let mut scroll_steps = 0usize;
    let initial_frame = session
        .latest_frame()
        .ok_or("full-Y requires an initial frame")?;
    let mut scroll_y = initial_frame.scroll_y;
    let max_scroll = (initial_frame.content_height.round() - viewport.logical_height()).max(0.0);
    scroll_to_bottom(session, &mut scroll_y, max_scroll, &mut scroll_steps)?;
    let actual_bottom = scroll_y == max_scroll;
    if !actual_bottom {
        return Err("full-Y did not reach the actual bottom clamp".into());
    }
    let full_y = phase_snapshot(full_y_start, session)?;
    prepare_top_scroll(session, max_scroll)?;
    Ok((full_y, scroll_steps, scroll_y, max_scroll))
}

fn scroll_to_bottom(
    session: &mut HtmlBrowserSession,
    scroll_y: &mut f32,
    max_scroll: f32,
    scroll_steps: &mut usize,
) -> Result<(), Box<dyn std::error::Error>> {
    while *scroll_y < max_scroll {
        session.dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: SCROLL_STEP,
        })?;
        *scroll_steps += 1;
        let next = session
            .latest_frame()
            .ok_or("full-Y scroll did not produce a frame")?
            .scroll_y;
        if next <= *scroll_y {
            return Err("full-Y scroll made no progress before bottom".into());
        }
        *scroll_y = next;
    }
    Ok(())
}

fn prepare_top_scroll(
    session: &mut HtmlBrowserSession,
    max_scroll: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    session.dispatch_input(HtmlBrowserInput::Scroll {
        delta_x: 0.0,
        delta_y: -max_scroll,
    })?;
    let top_scroll = session
        .latest_frame()
        .ok_or("full-Y preparation did not produce a top frame")?
        .scroll_y;
    if top_scroll != 0.0 {
        return Err("full-Y preparation did not return to scroll origin".into());
    }
    Ok(())
}

fn pointer_click_inputs(x: f32, y: f32) -> [HtmlBrowserInput; POINTER_EVENT_SEQUENCE_LENGTH] {
    [
        HtmlBrowserInput::PointerMove { x, y },
        HtmlBrowserInput::PointerDown { x, y, button: 0 },
        HtmlBrowserInput::PointerUp { x, y, button: 0 },
    ]
}
