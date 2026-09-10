mod drawio;
mod html;
mod html_browser;
mod html_css;
mod html_css_cascade;
mod html_css_rule;
mod html_css_selector;
mod html_css_sources;
mod html_debug_trace;
mod html_document;
mod html_document_fragment;
mod html_dom_helpers;
mod html_interactive;
mod html_runtime;
#[cfg(test)]
#[path = "html_runtime_dom_error_tests.rs"]
mod html_runtime_dom_error_tests;
#[cfg(test)]
mod html_runtime_interaction_tests;
#[cfg(test)]
mod html_runtime_intersection_observer_tests;
#[cfg(test)]
mod html_runtime_tests;
mod html_snapshot;
mod html_subresources;
mod html_table;
mod mathjax;
mod mermaid;
mod plantuml;

pub use drawio::DrawioRenderer;
pub use html::{HtmlRenderInput, HtmlRenderOutput, HtmlRenderer};
pub use html_browser::{
    HTML_BROWSER_MAX_SOURCE_BYTES, HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserInput,
    HtmlBrowserNavigation, HtmlBrowserNavigationEvent, HtmlBrowserOrigin, HtmlBrowserPixelFormat,
    HtmlBrowserSession, HtmlBrowserSessionState, HtmlBrowserSource, HtmlBrowserViewport,
    HtmlRuntime, HtmlRuntimeSession,
};
pub use html_runtime::HtmlRuntimeError;
#[cfg(test)]
pub(crate) use html_runtime::StaticHtmlRuntime;
#[cfg(test)]
pub(crate) use html_runtime::{HtmlNodeId, HtmlRuntimeEvent};
pub use mathjax::MathJaxRenderer;
pub use mermaid::MermaidRenderer;
pub use plantuml::PlantUmlRenderer;

#[cfg(test)]
mod tests {
    use super::{DrawioRenderer, MathJaxRenderer, MermaidRenderer, PlantUmlRenderer};
    use crate::renderer::api::{
        DiagramKind, RenderConfig, RenderContext, RenderInput, RenderPolicy, Renderer,
    };

    #[test]
    fn renderers_reject_wrong_kind_before_runtime_execution() {
        let mermaid = MermaidRenderer::with_runtime_path("missing-mermaid.js".into());
        let drawio = DrawioRenderer::with_runtime_path("missing-drawio.js".into());
        let mathjax = MathJaxRenderer::with_runtime_path("missing-mathjax.js".into());
        let plantuml = PlantUmlRenderer::with_runtime_path("missing-plantuml.jar".into());

        assert!(mermaid.render(&input(DiagramKind::Drawio, "x")).is_err());
        assert!(drawio.render(&input(DiagramKind::Mermaid, "x")).is_err());
        assert!(mathjax.render(&input(DiagramKind::Mermaid, "x")).is_err());
        assert!(plantuml.render(&input(DiagramKind::Mermaid, "x")).is_err());
        assert!(mermaid.render(&input(DiagramKind::Mermaid, " ")).is_ok());
        assert!(drawio.render(&input(DiagramKind::Drawio, "x")).is_err());
        assert!(mathjax.render(&input(DiagramKind::MathJax, " ")).is_ok());
        assert!(plantuml.render(&input(DiagramKind::PlantUml, " ")).is_ok());
        assert!(plantuml.render(&input(DiagramKind::PlantUml, "x")).is_ok());
    }

    fn input(kind: DiagramKind, source: &str) -> RenderInput {
        RenderInput {
            kind,
            source: source.to_string(),
            config: RenderConfig::default(),
            policy: RenderPolicy::default(),
            context: RenderContext::default(),
        }
    }
}
