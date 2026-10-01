use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
use crate::renderer::backends::html_browser::HtmlBrowserSource;
use crate::renderer::backends::html_debug_trace::HtmlDebugTrace;
use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_runtime::dom_state::HtmlDomBridgeState;
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;
use crate::renderer::backends::html_subresources::{HtmlDocumentResources, HtmlSubresourceLoader};

use super::{StaticHtmlRuntime, StaticHtmlRuntimeSession};

impl StaticHtmlRuntime {
    pub(in crate::renderer::backends) fn start_interactive_traced(
        &self,
        source: &HtmlBrowserSource,
        trace: &HtmlDebugTrace,
    ) -> Result<StaticHtmlRuntimeSession, HtmlRuntimeError> {
        let (document, resources, resource_loader) = load_document(source, trace);
        let mut isolate = initialize_isolate(document, resource_loader, trace);
        let started = trace.start();
        let context = Self::execute_interactive_scripts(
            &mut isolate,
            &resources.scripts,
            resources.body_onload_script_index,
            source.origin.as_str(),
        )?;
        trace.finish(
            0,
            "script_execution",
            started,
            &[("scripts", resources.scripts.len())],
        );
        Ok(StaticHtmlRuntimeSession {
            context: Some(context),
            isolate: Some(isolate),
            external_stylesheets: resources.stylesheets,
        })
    }
}

fn load_document(
    source: &HtmlBrowserSource,
    trace: &HtmlDebugTrace,
) -> (HtmlDocument, HtmlDocumentResources, HtmlSubresourceLoader) {
    let started = trace.start();
    let mut document = HtmlDocument::parse(&source.raw_html);
    trace.finish(0, "dom_parse", started, &[]);
    let started = trace.start();
    let resources = HtmlSubresourceLoader::new(source).load(&mut document);
    let loader = HtmlSubresourceLoader::new(source);
    trace.finish(
        0,
        "subresource_load",
        started,
        &[
            ("stylesheets", resources.stylesheets.len()),
            ("scripts", resources.scripts.len()),
        ],
    );
    (document, resources, loader)
}

fn initialize_isolate(
    document: HtmlDocument,
    resource_loader: HtmlSubresourceLoader,
    trace: &HtmlDebugTrace,
) -> v8::OwnedIsolate {
    let started = trace.start();
    DiagramV8Runtime::ensure_initialized();
    let mut isolate = v8::Isolate::new(Default::default());
    isolate.set_slot(HtmlDomBridgeState::new_interactive(
        document,
        resource_loader,
    ));
    trace.finish(0, "v8_setup", started, &[]);
    isolate
}
