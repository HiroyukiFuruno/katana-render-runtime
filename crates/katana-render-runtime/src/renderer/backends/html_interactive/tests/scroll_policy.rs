use super::super::HtmlInteractiveSession;
use super::support::{TestResult, start, to_string};
use crate::renderer::backends::html_browser::{HtmlBrowserInput, HtmlBrowserOverflowPolicy};

const HIDDEN_OVERFLOW_DOCUMENT: &str = r#"<body style="margin:0;overflow-y:hidden">
<p id="result">initial</p><div style="height:1000px"></div>
<script>window.addEventListener('scroll', () => {
  document.getElementById('result').textContent = 'scrolled';
});</script></body>"#;
const OBSERVER_SCROLL_DOCUMENT: &str = r#"<body style="margin:0">
<div style="height:400px"></div><div id="target" style="height:40px">target</div>
<script>new IntersectionObserver(entries => {
  if (entries[0].isIntersecting) throw new Error('intersection callback failed');
  document.getElementById('target').setAttribute('data-observer', 'waiting');
}).observe(document.getElementById('target'));</script></body>"#;
const OBSERVER_SCROLL_DELTA: f32 = 300.0;

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

#[test]
fn intersection_observer_exception_reaches_host_scroll_dispatch() -> TestResult {
    let mut session = start(OBSERVER_SCROLL_DOCUMENT)?;
    let initial = session.runtime.snapshot().map_err(to_string)?;
    assert!(initial.contains("data-observer=\"waiting\""), "{initial}");
    let error = session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: OBSERVER_SCROLL_DELTA,
        })
        .err()
        .ok_or_else(|| "intersecting observer error was not propagated".to_string())?;
    assert!(
        error.to_string().contains("intersection callback failed"),
        "{error}"
    );
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
