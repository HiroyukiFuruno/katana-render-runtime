use super::api::{DiagramKind, RenderError, RenderInput, RenderOutput, Renderer};
use super::output::RenderOutputFactory;
use super::runtime::RuntimeDescriptor;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::{
    DiagramBlock, drawio_renderer::DrawioRendererOps, mermaid_renderer::MermaidRenderOps,
};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MermaidRenderer {
    runtime_path: PathBuf,
}

impl MermaidRenderer {
    pub fn with_runtime_path(runtime_path: PathBuf) -> Self {
        Self { runtime_path }
    }
}

impl Renderer for MermaidRenderer {
    fn render(&self, input: &RenderInput) -> Result<RenderOutput, RenderError> {
        if input.kind != DiagramKind::Mermaid {
            return Err(RenderError::UnsupportedKind);
        }
        RenderOutputFactory::from_diagram_result(
            input,
            self.render_block(input),
            RuntimeDescriptor::mermaid(),
        )
    }
}

impl MermaidRenderer {
    fn render_block(&self, input: &RenderInput) -> crate::markdown::DiagramResult {
        let block = DiagramBlock {
            kind: crate::markdown::DiagramKind::Mermaid,
            source: input.source.clone(),
        };
        let preset = DiagramColorPreset::for_render_input(input);
        MermaidRenderOps::render_mermaid_with_runtime_path(&block, &self.runtime_path, &preset)
    }
}

#[derive(Debug, Clone)]
pub struct DrawioRenderer {
    runtime_path: PathBuf,
}

impl DrawioRenderer {
    pub fn with_runtime_path(runtime_path: PathBuf) -> Self {
        Self { runtime_path }
    }
}

impl Renderer for DrawioRenderer {
    fn render(&self, input: &RenderInput) -> Result<RenderOutput, RenderError> {
        if input.kind != DiagramKind::Drawio {
            return Err(RenderError::UnsupportedKind);
        }
        RenderOutputFactory::from_diagram_result(
            input,
            self.render_block(input),
            RuntimeDescriptor::drawio(),
        )
    }
}

impl DrawioRenderer {
    fn render_block(&self, input: &RenderInput) -> crate::markdown::DiagramResult {
        let block = DiagramBlock {
            kind: crate::markdown::DiagramKind::DrawIo,
            source: input.source.clone(),
        };
        let preset = DiagramColorPreset::for_render_input(input);
        DrawioRendererOps::render_drawio_with_runtime_path(&block, &self.runtime_path, &preset)
    }
}

#[cfg(test)]
mod tests {
    use super::{DrawioRenderer, MermaidRenderer};
    use crate::renderer::api::{
        DiagramKind, RenderConfig, RenderContext, RenderInput, RenderPolicy, Renderer,
    };

    #[test]
    fn renderers_reject_wrong_kind_before_runtime_execution() {
        let mermaid = MermaidRenderer::with_runtime_path("missing-mermaid.js".into());
        let drawio = DrawioRenderer::with_runtime_path("missing-drawio.js".into());

        assert!(mermaid.render(&input(DiagramKind::Drawio, "x")).is_err());
        assert!(drawio.render(&input(DiagramKind::Mermaid, "x")).is_err());
        assert!(mermaid.render(&input(DiagramKind::Mermaid, " ")).is_ok());
        assert!(drawio.render(&input(DiagramKind::Drawio, "x")).is_err());
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
