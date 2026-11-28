use super::*;

const FRAGMENT_CLICK_COORDINATE: f32 = 10.0;

#[test]
fn propagated_root_clip_uses_hidden_scroll_metrics_and_allows_fragment_scroll() -> TestResult {
    for case in ROOT_CLIP_CASES {
        for scale in [1.0_f32, 2.0_f32] {
            assert_root_clip_case(case, scale)?;
        }
    }
    Ok(())
}

fn assert_root_clip_case(case: RootClipCase, scale: f32) -> TestResult {
    let mut session = open_root_clip_case(case, scale)?;
    let initial_generation = assert_initial_root_clip_metrics(&mut session, case)?;
    click_fragment(&mut session, scale)?;
    assert_fragment_scroll_metrics(&session, initial_generation, case)
}

fn open_root_clip_case(case: RootClipCase, scale: f32) -> TestResult<HtmlBrowserSession> {
    let html = format!(
        "<!doctype html><html style=\"{}\"><body style=\"margin:0;width:900px;height:700px;{}\"><a href=\"#target\" style=\"position:absolute;left:8px;top:8px;width:80px;height:32px\">Jump</a><div id=\"target\" style=\"position:absolute;left:8px;top:520px;width:20px;height:20px\">Target</div></body></html>",
        case.html_overflow, case.body_overflow
    );
    let source = HtmlBrowserSource::new(html, SOURCE_ORIGIN).map_err(|error| error.to_string())?;
    HtmlBrowserSession::new(source, viewport_for_scale(scale)?).map_err(|error| error.to_string())
}

fn assert_initial_root_clip_metrics(
    session: &mut HtmlBrowserSession,
    case: RootClipCase,
) -> TestResult<u64> {
    let (initial_frame, initial_metrics) = session
        .latest_frame_with_metrics()
        .ok_or_else(|| format!("{}: initial frame and metrics must be paired", case.label))?;
    let initial_generation = initial_frame.generation;
    assert_eq!(
        initial_generation, initial_metrics.generation,
        "{}",
        case.label
    );
    assert_root_clip_policy(initial_metrics, case);
    assert_root_clip_scrollability(initial_metrics, case);
    consume_initial_root_clip_frame(session, case)?;
    Ok(initial_generation)
}

fn assert_root_clip_policy(metrics: &HtmlBrowserScrollMetrics, case: RootClipCase) {
    assert_eq!(metrics.overflow_x, case.expected_x, "{}", case.label);
    assert_eq!(metrics.overflow_y, case.expected_y, "{}", case.label);
}

fn assert_root_clip_scrollability(metrics: &HtmlBrowserScrollMetrics, case: RootClipCase) {
    assert!(metrics.max_scroll_x() > 0.0, "{}: x range", case.label);
    assert!(metrics.max_scroll_y() > 0.0, "{}: y range", case.label);
    assert!(
        !metrics.horizontal_scrollbar_visible(),
        "{}: x scrollbar",
        case.label
    );
    assert_eq!(
        metrics.vertical_scrollbar_visible(),
        case.expected_y == HtmlBrowserOverflowPolicy::Auto,
        "{}: y scrollbar",
        case.label
    );
}

fn consume_initial_root_clip_frame(
    session: &mut HtmlBrowserSession,
    case: RootClipCase,
) -> TestResult {
    session
        .take_frame_update_with_metrics()
        .map(|_| ())
        .ok_or_else(|| format!("{}: initial update missing", case.label))
}

fn click_fragment(session: &mut HtmlBrowserSession, scale: f32) -> TestResult {
    let (x, y) = (
        FRAGMENT_CLICK_COORDINATE * scale,
        FRAGMENT_CLICK_COORDINATE * scale,
    );
    for event in [
        HtmlBrowserInput::PointerDown { x, y, button: 0 },
        HtmlBrowserInput::PointerUp { x, y, button: 0 },
    ] {
        session
            .dispatch_input(event)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn assert_fragment_scroll_metrics(
    session: &HtmlBrowserSession,
    initial_generation: u64,
    case: RootClipCase,
) -> TestResult {
    let (frame, metrics) = session
        .latest_frame_with_metrics()
        .ok_or_else(|| format!("{}: scrolled frame and metrics must be paired", case.label))?;
    assert_fragment_frame_metrics(frame, metrics, initial_generation, case);
    assert_fragment_origin(session, case);
    Ok(())
}

fn assert_fragment_frame_metrics(
    frame: &HtmlBrowserFrame,
    metrics: &HtmlBrowserScrollMetrics,
    initial_generation: u64,
    case: RootClipCase,
) {
    assert!(
        frame.generation > initial_generation,
        "{}: generation",
        case.label
    );
    assert_eq!(
        frame.generation, metrics.generation,
        "{}: paired generation",
        case.label
    );
    assert!(
        metrics.scroll_y > 0.0,
        "{}: programmatic scroll position",
        case.label
    );
    assert_eq!(
        frame.scroll_y, metrics.scroll_y,
        "{}: frame scroll position",
        case.label
    );
}

fn assert_fragment_origin(session: &HtmlBrowserSession, case: RootClipCase) {
    assert!(
        session.source().origin.as_str().ends_with("#target"),
        "{}: fragment",
        case.label
    );
}
