mod drawio;
mod mathjax;
mod mermaid;
mod plantuml;

pub use drawio::DrawioRenderer;
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
