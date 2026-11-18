use super::{ScriptlessDocument, StaticHtmlRuntime};
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
use crate::private_v8 as v8;
use crate::renderer::backends::html_subresources::HtmlSubresourceLoader;

use super::super::dom_state::{HtmlDomBridgeState, LayoutMetricWithIntersection};
use super::super::script::{evaluate, install_dom_bridge, update_location};
use super::super::types::HtmlRuntimeError;
use super::session_interaction::set_focused_element;

pub(super) struct ScriptlessObserver {
    pub(super) document_url: String,
    pub(super) focused_node: Option<u64>,
    geometry: Option<ObservedGeometry>,
    resource_loader: Option<HtmlSubresourceLoader>,
    javascript: Option<ObserverJavascript>,
}

struct ObservedGeometry {
    width: f32,
    height: f32,
    scroll_offset: (f32, f32),
    boxes: Vec<LayoutMetricWithIntersection>,
}

struct ObserverJavascript {
    context: v8::Global<v8::Context>,
    isolate: v8::OwnedIsolate,
}

impl ScriptlessObserver {
    pub(super) fn new(document_url: &str, resource_loader: HtmlSubresourceLoader) -> Self {
        Self {
            document_url: document_url.to_owned(),
            focused_node: None,
            geometry: None,
            resource_loader: Some(resource_loader),
            javascript: None,
        }
    }

    pub(super) fn record_geometry(
        &mut self,
        width: f32,
        height: f32,
        scroll_offset: (f32, f32),
        boxes: impl IntoIterator<Item = LayoutMetricWithIntersection>,
    ) {
        self.geometry = Some(ObservedGeometry {
            width,
            height,
            scroll_offset,
            boxes: boxes.into_iter().collect(),
        });
    }

    pub(super) fn prepare(
        &mut self,
        document: &ScriptlessDocument,
    ) -> Result<(&mut v8::OwnedIsolate, &v8::Global<v8::Context>), String> {
        if self.javascript.is_none() {
            let loader = self
                .resource_loader
                .take()
                .ok_or("missing observer resource loader")?;
            self.javascript = Some(
                ObserverJavascript::new(document, loader, &self.document_url)
                    .map_err(|error| error.to_string())?,
            );
        }
        let javascript = self
            .javascript
            .as_mut()
            .ok_or("missing observer JavaScript context")?;
        if let Some(geometry) = &self.geometry {
            geometry.apply(&javascript.isolate)?;
        }
        javascript
            .synchronize(&self.document_url, self.focused_node)
            .map_err(|error| error.to_string())?;
        Ok((&mut javascript.isolate, &javascript.context))
    }
}

impl ObservedGeometry {
    fn apply(&self, isolate: &v8::OwnedIsolate) -> Result<(), String> {
        let state = isolate
            .get_slot::<HtmlDomBridgeState>()
            .ok_or("missing observed DOM")?;
        state.set_layout_metrics_xy(
            self.width,
            self.height,
            self.scroll_offset,
            self.boxes.iter().cloned(),
        );
        Ok(())
    }
}

impl ObserverJavascript {
    fn new(
        document: &ScriptlessDocument,
        resource_loader: HtmlSubresourceLoader,
        document_url: &str,
    ) -> Result<Self, HtmlRuntimeError> {
        DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        isolate.set_slot(HtmlDomBridgeState::with_observed_document(
            ScriptlessDocument::clone(document),
            resource_loader,
        ));
        let context =
            StaticHtmlRuntime::with_new_context(&mut isolate, |context_scope, context| {
                v8::tc_scope!(let scope, &mut **context_scope);
                install_dom_bridge(scope, document_url)?;
                evaluate(
                    scope,
                    "krr-html-observed-ready-state",
                    "__krrDocumentReadyState = 'complete';",
                )
                .map(|()| v8::Global::new(scope, context))
            })?;
        Ok(Self { context, isolate })
    }

    fn synchronize(
        &mut self,
        document_url: &str,
        focused_node: Option<u64>,
    ) -> Result<(), HtmlRuntimeError> {
        v8::scope!(let handle_scope, &mut self.isolate);
        let context = v8::Local::new(handle_scope, &self.context);
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        update_location(scope, document_url)?;
        set_focused_element(scope, focused_node)
    }
}
