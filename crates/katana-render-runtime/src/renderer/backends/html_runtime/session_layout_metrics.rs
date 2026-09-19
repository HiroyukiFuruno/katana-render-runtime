use super::super::script::{
    check_bridge_error, dom_state_unavailable_error, evaluate, evaluate_value,
    perform_microtask_checkpoint,
};
use super::StaticHtmlRuntimeSession;
use super::session_interaction::discarded_runtime_error;
use crate::renderer::backends::html_runtime::dom_state::HtmlDomBridgeState;
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;

type LayoutMetric = (u64, f32, f32, f32, f32, f32);

impl StaticHtmlRuntimeSession {
    #[cfg(test)]
    pub(in crate::renderer::backends) fn update_layout_metrics(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scroll_y: f32,
        boxes: impl IntoIterator<Item = (u64, f32, f32, f32, f32, f32)>,
    ) -> Result<bool, HtmlRuntimeError> {
        self.update_layout_metrics_with_intersection_metadata(
            viewport_width,
            viewport_height,
            scroll_y,
            boxes,
            [],
        )
    }

    pub(in crate::renderer::backends) fn update_layout_metrics_with_intersection_metadata(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scroll_y: f32,
        boxes: impl IntoIterator<Item = (u64, f32, f32, f32, f32, f32)>,
        metadata: impl IntoIterator<Item = (u64, String)>,
    ) -> Result<bool, HtmlRuntimeError> {
        self.update_layout_metrics_from_boxes(
            viewport_width,
            viewport_height,
            scroll_y,
            boxes.into_iter().collect(),
            metadata.into_iter().collect(),
        )
    }

    fn update_layout_metrics_from_boxes(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scroll_y: f32,
        boxes: Vec<LayoutMetric>,
        metadata: Vec<(u64, String)>,
    ) -> Result<bool, HtmlRuntimeError> {
        self.set_layout_metrics(viewport_width, viewport_height, scroll_y, boxes, metadata)?;
        let observer_work = self.has_intersection_observer_work();
        if matches!(observer_work, Err(HtmlRuntimeError::ExecutionTimeout)) {
            self.discard();
        }
        if !observer_work? {
            return Ok(false);
        }
        let before = self.snapshot()?;
        let refresh_result = self.refresh_intersection_observers();
        if matches!(refresh_result, Err(HtmlRuntimeError::ExecutionTimeout)) {
            self.discard();
        }
        refresh_result?;
        self.snapshot_changed(&before)
    }

    fn has_intersection_observer_work(&mut self) -> Result<bool, HtmlRuntimeError> {
        let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
        let context = self.context.as_ref().ok_or_else(discarded_runtime_error)?;
        v8::scope!(let handle_scope, isolate);
        let context = v8::Local::new(handle_scope, context);
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        let has_work = evaluate_value(
            scope,
            "krr-html-intersection-observer-work",
            "globalThis.__krrPrepareIntersectionObservers();",
        )?
        .is_true();
        check_bridge_error(scope)?;
        Ok(has_work)
    }

    fn set_layout_metrics(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scroll_y: f32,
        boxes: Vec<LayoutMetric>,
        metadata: Vec<(u64, String)>,
    ) -> Result<(), HtmlRuntimeError> {
        let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
        let state = isolate
            .get_slot::<HtmlDomBridgeState>()
            .ok_or_else(dom_state_unavailable_error)?;
        state.set_layout_metrics(viewport_width, viewport_height, scroll_y, boxes);
        state.set_intersection_metadata(metadata);
        Ok(())
    }

    fn refresh_intersection_observers(&mut self) -> Result<(), HtmlRuntimeError> {
        let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
        let context = self.context.as_ref().ok_or_else(discarded_runtime_error)?;
        v8::scope!(let handle_scope, isolate);
        let context = v8::Local::new(handle_scope, context);
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        evaluate(
            scope,
            "krr-html-intersection-layout-sync",
            "globalThis.__krrRefreshIntersectionObservers();",
        )
        .and_then(|()| perform_microtask_checkpoint(scope))
        .and_then(|()| check_bridge_error(scope))
    }

    fn snapshot_changed(&self, before: &str) -> Result<bool, HtmlRuntimeError> {
        Ok(before != self.snapshot()?)
    }
}

#[cfg(test)]
mod tests {
    use super::super::StaticHtmlRuntime;
    use super::{HtmlRuntimeError, LayoutMetric};

    fn start(source: &str) -> super::StaticHtmlRuntimeSession {
        let start = StaticHtmlRuntime.start(source);
        assert!(start.is_ok());
        let mut sessions = start.into_iter().collect::<Vec<_>>();
        sessions.remove(0)
    }

    #[test]
    fn layout_metrics_surfaces_observer_refresh_exceptions() {
        let mut session = start(
            "<p id=target>target</p><script>new IntersectionObserver(() => {}).observe(document.getElementById('target')); globalThis.__krrRefreshIntersectionObservers = () => { throw new Error('refresh'); };</script>",
        );
        let result = session.update_layout_metrics(320.0, 240.0, 0.0, []);
        assert!(matches!(
            result,
            Err(HtmlRuntimeError::JavaScriptException(message)) if message.contains("refresh")
        ));
    }

    #[test]
    fn layout_metrics_timeout_discards_the_runtime() {
        let mut session = start(
            "<p id=target>target</p><script>new IntersectionObserver(() => {}).observe(document.getElementById('target')); globalThis.__krrRefreshIntersectionObservers = () => { for (;;) {} };</script>",
        );

        let result = session.update_layout_metrics(320.0, 240.0, 0.0, []);

        assert_eq!(result, Err(HtmlRuntimeError::ExecutionTimeout));
        assert!(session.isolate.is_none());
        assert!(session.context.is_none());
        assert!(matches!(
            session.update_layout_metrics(320.0, 240.0, 0.0, []),
            Err(HtmlRuntimeError::DomBridge(_))
        ));
    }

    #[test]
    fn intersection_observer_defers_initial_notification_until_layout_metrics_exist()
    -> Result<(), HtmlRuntimeError> {
        let mut session = start(
            r#"<p id="target">target</p><script>
                let notifications = 0;
                new IntersectionObserver((entries) => {
                    notifications += 1;
                    entries[0].target.setAttribute("data-notifications", notifications);
                }).observe(document.getElementById("target"));
            </script>"#,
        );

        assert!(
            !session.snapshot()?.contains("data-notifications"),
            "observer must not receive synthetic geometry before the first layout metrics update"
        );

        session.update_layout_metrics(320.0, 240.0, 0.0, [])?;

        assert!(
            session.snapshot()?.contains("data-notifications=\"1\""),
            "first layout metrics update must notify the observer"
        );
        Ok(())
    }

    #[test]
    fn intersection_observer_reobserved_after_disconnect_is_refreshed()
    -> Result<(), HtmlRuntimeError> {
        let mut session = start(
            r#"<p id="target">target</p><script>
                let notifications = 0;
                const observer = new IntersectionObserver((entries) => {
                    notifications += 1;
                    entries[0].target.setAttribute("data-notifications", notifications);
                });
                const target = document.getElementById("target");
                observer.observe(target);
                observer.disconnect();
                observer.observe(target);
            </script>"#,
        );

        session.update_layout_metrics(320.0, 240.0, 0.0, [])?;

        assert!(
            session.snapshot()?.contains("data-notifications=\"1\""),
            "re-observing after disconnect must restore refresh registration"
        );
        Ok(())
    }

    #[test]
    fn intersection_observer_distinguishes_touching_and_separated_zero_area_rectangles()
    -> Result<(), HtmlRuntimeError> {
        let mut session = start(intersection_observer_geometry_source());
        let boxes = observer_geometry_boxes(&mut session)?;
        session.update_layout_metrics(100.0, 100.0, 0.0, boxes)?;

        let snapshot = session.snapshot()?;
        for (id, expected) in [
            ("touching", "true:0"),
            ("zero", "true:1"),
            ("separated", "false:0"),
        ] {
            assert!(
                snapshot.contains(&format!(r#"id="{id}" data-intersection="{expected}""#)),
                "expected #{id} to report {expected}: {snapshot}"
            );
        }
        Ok(())
    }

    fn observer_geometry_boxes(
        session: &mut super::super::StaticHtmlRuntimeSession,
    ) -> Result<[LayoutMetric; 3], HtmlRuntimeError> {
        Ok([
            (node_id(session, "touching")?, 0.0, 100.0, 10.0, 10.0, 0.0),
            (node_id(session, "zero")?, 20.0, 20.0, 0.0, 0.0, 0.0),
            (node_id(session, "separated")?, 150.0, 20.0, 0.0, 0.0, 0.0),
        ])
    }

    fn intersection_observer_geometry_source() -> &'static str {
        r#"<p id="touching">touching</p><p id="zero">zero</p><p id="separated">separated</p><script>
            const observer = new IntersectionObserver((entries) => {
                for (const entry of entries) {
                    entry.target.setAttribute(
                        "data-intersection",
                        `${entry.isIntersecting}:${entry.intersectionRatio}`,
                    );
                }
            });
            for (const id of ["touching", "zero", "separated"]) {
                observer.observe(document.getElementById(id));
            }
        </script>"#
    }

    fn node_id(
        session: &mut super::super::StaticHtmlRuntimeSession,
        id: &str,
    ) -> Result<u64, HtmlRuntimeError> {
        session
            .node_for_element_id(id)
            .map(|node| node.0)
            .ok_or_else(|| HtmlRuntimeError::DomBridge(format!("missing test node #{id}")))
    }

    #[test]
    fn node_id_reports_a_missing_test_fixture() {
        let mut session = start("<p>content</p>");
        assert!(matches!(
            node_id(&mut session, "missing"),
            Err(HtmlRuntimeError::DomBridge(message)) if message == "missing test node #missing"
        ));
    }

    #[test]
    fn layout_metrics_rejects_a_discarded_runtime() {
        let mut session = start("<p>content</p>");
        session.isolate = None;
        let result = session.update_layout_metrics(320.0, 240.0, 0.0, []);
        assert!(matches!(result, Err(HtmlRuntimeError::DomBridge(_))));
    }

    #[test]
    fn layout_metrics_rejects_a_runtime_without_context() {
        let mut session = start("<p>content</p>");
        session.context = None;
        let result = session.update_layout_metrics(320.0, 240.0, 0.0, []);
        assert!(matches!(result, Err(HtmlRuntimeError::DomBridge(_))));
    }

    #[test]
    fn snapshot_changed_surfaces_a_discarded_runtime() {
        let mut session = start("<p>content</p>");
        session.isolate = None;
        let result = session.snapshot_changed("");
        assert!(matches!(result, Err(HtmlRuntimeError::DomBridge(_))));
    }

    #[test]
    fn observer_work_rejects_a_discarded_runtime() {
        let mut session = start("<p>content</p>");
        session.isolate = None;
        assert!(matches!(
            session.has_intersection_observer_work(),
            Err(HtmlRuntimeError::DomBridge(_))
        ));
    }
}
