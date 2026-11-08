use super::super::HtmlInteractiveSession;
use super::support::{TestResult, start, to_string};
use crate::renderer::backends::html_browser::{HtmlBrowserInput, HtmlBrowserOverflowPolicy};

const HIDDEN_OVERFLOW_DOCUMENT: &str = r#"<body style="margin:0;overflow-y:hidden">
<p id="result">initial</p><div style="height:1000px"></div>
<script>window.addEventListener('scroll', () => {
  document.getElementById('result').textContent = 'scrolled';
});</script></body>"#;

#[test]
fn text_without_focused_input_preserves_dom_and_frame() -> TestResult {
    let mut session = start("<input id=field value=initial><p id=result>unchanged</p>")?;
    let before = session.runtime.snapshot().map_err(to_string)?;
    let generation = session
        .latest_frame()
        .ok_or_else(|| "initial frame must exist".to_string())?
        .generation;
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: "ignored".to_owned(),
        })
        .map_err(to_string)?;
    assert_eq!(session.runtime.snapshot().map_err(to_string)?, before);
    assert_eq!(
        session.latest_frame().map(|frame| frame.generation),
        Some(generation)
    );
    Ok(())
}

#[test]
fn hidden_viewport_overflow_rejects_host_scroll_and_skips_window_event() -> TestResult {
    let mut session = start(HIDDEN_OVERFLOW_DOCUMENT)?;
    let generation = session
        .latest_frame()
        .ok_or_else(|| "initial frame must exist".to_string())?
        .generation;

    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 120.0,
        })
        .map_err(to_string)?;

    assert_scroll_remains_blocked(&session, generation)?;
    Ok(())
}

fn assert_scroll_remains_blocked(session: &HtmlInteractiveSession, generation: u64) -> TestResult {
    let metrics = session
        .latest_scroll_metrics()
        .ok_or_else(|| "scroll metrics must exist".to_string())?;
    assert_eq!(metrics.overflow_y, HtmlBrowserOverflowPolicy::Hidden);
    assert_eq!(metrics.scroll_y, 0.0);
    assert_eq!(
        session.latest_frame().map(|frame| frame.generation),
        Some(generation)
    );
    assert!(
        session
            .runtime
            .snapshot()
            .map_err(to_string)?
            .contains(">initial<")
    );
    Ok(())
}
