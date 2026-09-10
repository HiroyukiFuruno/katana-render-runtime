use super::super::script::{
    check_bridge_error, dom_state_unavailable_error, evaluate, perform_microtask_checkpoint,
};
use super::StaticHtmlRuntimeSession;
use super::session_interaction::discarded_runtime_error;
use crate::renderer::backends::html_runtime::dom_state::HtmlDomBridgeState;
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;

type LayoutMetric = (u64, f32, f32, f32, f32, f32);

impl StaticHtmlRuntimeSession {
    pub(in crate::renderer::backends) fn update_layout_metrics(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scroll_y: f32,
        boxes: impl IntoIterator<Item = (u64, f32, f32, f32, f32, f32)>,
    ) -> Result<bool, HtmlRuntimeError> {
        self.update_layout_metrics_from_boxes(
            viewport_width,
            viewport_height,
            scroll_y,
            boxes.into_iter().collect(),
        )
    }

    fn update_layout_metrics_from_boxes(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scroll_y: f32,
        boxes: Vec<LayoutMetric>,
    ) -> Result<bool, HtmlRuntimeError> {
        let before = self.snapshot()?;
        {
            let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
            let state = isolate
                .get_slot::<HtmlDomBridgeState>()
                .ok_or_else(dom_state_unavailable_error)?;
            state.set_layout_metrics(viewport_width, viewport_height, scroll_y, boxes);

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
            .and_then(|()| check_bridge_error(scope))?;
        }
        self.snapshot_changed(&before)
    }

    fn snapshot_changed(&self, before: &str) -> Result<bool, HtmlRuntimeError> {
        Ok(before != self.snapshot()?)
    }
}

#[cfg(test)]
mod tests {
    use super::super::StaticHtmlRuntime;
    use super::HtmlRuntimeError;

    fn start(source: &str) -> super::StaticHtmlRuntimeSession {
        let start = StaticHtmlRuntime.start(source);
        assert!(start.is_ok());
        let mut sessions = start.into_iter().collect::<Vec<_>>();
        sessions.remove(0)
    }

    #[test]
    fn layout_metrics_surfaces_observer_refresh_exceptions() {
        let mut session = start(
            "<script>globalThis.__krrRefreshIntersectionObservers = () => { throw new Error('refresh'); };</script>",
        );
        let result = session.update_layout_metrics(320.0, 240.0, 0.0, []);
        assert!(matches!(
            result,
            Err(HtmlRuntimeError::JavaScriptException(message)) if message.contains("refresh")
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
}
