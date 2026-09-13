use super::super::StaticHtmlRuntime;
use super::super::types::{HtmlNodeId, HtmlRuntimeError, HtmlRuntimeEvent};
use super::StaticHtmlRuntimeSession;

type LayoutMetric = (u64, f32, f32, f32, f32, f32);
const OBSERVED_TARGET_COUNT: usize = 3;
const OBSERVED_TARGET_SIZE: f32 = 10.0;
const HIDDEN_TARGET_X: f32 = 20.0;
const REMOVED_TARGET_X: f32 = 40.0;

#[test]
fn intersection_observer_marks_hidden_and_removed_targets_not_intersecting()
-> Result<(), HtmlRuntimeError> {
    let mut session = start(hidden_and_removed_observer_source());
    let visible = node_id(&mut session, "visible")?;
    let hidden = node_id(&mut session, "hidden")?;
    let removed = node_id(&mut session, "removed")?;
    session.update_layout_metrics(
        100.0,
        100.0,
        0.0,
        observed_target_boxes(visible, hidden, removed),
    )?;
    assert_observer_transition(&session, "true:1", "true:1")?;
    session.dispatch_window_scroll()?;
    session.update_layout_metrics(100.0, 100.0, 0.0, [(visible, 0.0, 0.0, 10.0, 10.0, 0.0)])?;
    assert_observer_transition(&session, "false:0", "false:0")
}

fn start(source: &str) -> StaticHtmlRuntimeSession {
    let start = StaticHtmlRuntime.start(source);
    assert!(start.is_ok());
    let mut sessions = start.into_iter().collect::<Vec<_>>();
    sessions.remove(0)
}

fn node_id(session: &mut StaticHtmlRuntimeSession, id: &str) -> Result<u64, HtmlRuntimeError> {
    session
        .node_for_element_id(id)
        .map(|node| node.0)
        .ok_or_else(|| HtmlRuntimeError::DomBridge(format!("missing test node #{id}")))
}

#[test]
fn node_id_rejects_a_missing_observer_fixture() {
    let mut session = start("<p>content</p>");
    assert!(matches!(
        node_id(&mut session, "missing"),
        Err(HtmlRuntimeError::DomBridge(message)) if message == "missing test node #missing"
    ));
}

#[test]
fn empty_observer_registry_skips_the_refresh_callback() -> Result<(), HtmlRuntimeError> {
    for source in [unobserved_observer_source(), disconnected_observer_source()] {
        let mut session = start(source);
        assert!(!session.update_layout_metrics(100.0, 100.0, 0.0, [])?);
    }
    Ok(())
}

#[test]
fn metrics_remain_ready_when_an_observer_is_registered_after_an_empty_refresh()
-> Result<(), HtmlRuntimeError> {
    let mut session = start(late_observer_source());
    let target = node_id(&mut session, "target")?;

    assert!(!session.update_layout_metrics(
        100.0,
        100.0,
        0.0,
        [(target, 0.0, 0.0, 10.0, 10.0, 0.0)]
    )?);
    session.dispatch(HtmlRuntimeEvent::Click {
        target: HtmlNodeId(target),
    })?;

    assert!(
        session
            .snapshot()?
            .contains(r#"data-intersection="true:1""#)
    );
    Ok(())
}

#[test]
fn observer_work_preparation_surfaces_exceptions() {
    let mut session = start(prepare_exception_source());
    let result = session.update_layout_metrics(100.0, 100.0, 0.0, []);
    assert!(matches!(
        result,
        Err(HtmlRuntimeError::JavaScriptException(message)) if message.contains("prepare")
    ));
}

#[test]
fn observer_work_preparation_timeout_discards_the_runtime() {
    let mut session = start(prepare_timeout_source());
    let result = session.update_layout_metrics(100.0, 100.0, 0.0, []);
    assert_eq!(result, Err(HtmlRuntimeError::ExecutionTimeout));
    assert!(session.isolate.is_none());
    assert!(session.context.is_none());
}

fn observed_target_boxes(
    visible: u64,
    hidden: u64,
    removed: u64,
) -> [LayoutMetric; OBSERVED_TARGET_COUNT] {
    [
        observed_target_box(visible, 0.0),
        observed_target_box(hidden, HIDDEN_TARGET_X),
        observed_target_box(removed, REMOVED_TARGET_X),
    ]
}

fn observed_target_box(node_id: u64, x: f32) -> LayoutMetric {
    (
        node_id,
        x,
        0.0,
        OBSERVED_TARGET_SIZE,
        OBSERVED_TARGET_SIZE,
        0.0,
    )
}

fn assert_observer_transition(
    session: &StaticHtmlRuntimeSession,
    hidden: &str,
    removed: &str,
) -> Result<(), HtmlRuntimeError> {
    let snapshot = session.snapshot()?;
    assert!(snapshot.contains(&format!(r#"data-hidden="{hidden}""#)));
    assert!(snapshot.contains(&format!(r#"data-removed="{removed}""#)));
    Ok(())
}

fn hidden_and_removed_observer_source() -> &'static str {
    r#"<p id="visible">visible</p><p id="hidden">hidden</p><p id="removed">removed</p><p id="result"></p><script>
        const result = document.getElementById("result");
        const hidden = document.getElementById("hidden");
        const removed = document.getElementById("removed");
        const observer = new IntersectionObserver((entries) => {
            for (const entry of entries) result.setAttribute(`data-${entry.target.getAttribute("id")}`, `${entry.isIntersecting}:${entry.intersectionRatio}`);
        });
        for (const target of [document.getElementById("visible"), hidden, removed]) observer.observe(target);
        window.addEventListener("scroll", () => { hidden.style.display = "none"; removed.remove(); });
    </script>"#
}

fn unobserved_observer_source() -> &'static str {
    r#"<p id="target">target</p><script>
        const observer = new IntersectionObserver(() => {});
        observer.observe(document.getElementById("target"));
        observer.unobserve(document.getElementById("target"));
        globalThis.__krrRefreshIntersectionObservers = () => { throw new Error("unexpected refresh"); };
    </script>"#
}

fn disconnected_observer_source() -> &'static str {
    r#"<p id="target">target</p><script>
        const observer = new IntersectionObserver(() => {});
        observer.observe(document.getElementById("target"));
        observer.disconnect();
        globalThis.__krrRefreshIntersectionObservers = () => { throw new Error("unexpected refresh"); };
    </script>"#
}

fn late_observer_source() -> &'static str {
    r#"<p id="target">target</p><script>
        const target = document.getElementById("target");
        const observer = new IntersectionObserver((entries) => {
            target.setAttribute("data-intersection", `${entries[0].isIntersecting}:${entries[0].intersectionRatio}`);
        });
        target.addEventListener("click", () => observer.observe(target));
    </script>"#
}

fn prepare_exception_source() -> &'static str {
    r#"<script>globalThis.__krrPrepareIntersectionObservers = () => { throw new Error("prepare"); };</script>"#
}

fn prepare_timeout_source() -> &'static str {
    r#"<script>globalThis.__krrPrepareIntersectionObservers = () => { for (;;) {} };</script>"#
}
