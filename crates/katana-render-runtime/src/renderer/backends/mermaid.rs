use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::{DiagramBlock, mermaid_renderer::MermaidRenderOps};
use crate::renderer::api::{DiagramKind, RenderError, RenderInput, RenderOutput, Renderer};
use crate::renderer::output::RenderOutputFactory;
use crate::renderer::runtime::RuntimeDescriptor;
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
