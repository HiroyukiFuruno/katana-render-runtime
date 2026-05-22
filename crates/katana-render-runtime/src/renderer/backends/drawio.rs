use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::{DiagramBlock, drawio_renderer::DrawioRendererOps};
use crate::renderer::api::{DiagramKind, RenderError, RenderInput, RenderOutput, Renderer};
use crate::renderer::output::RenderOutputFactory;
use crate::renderer::runtime::RuntimeDescriptor;
use std::path::PathBuf;

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
