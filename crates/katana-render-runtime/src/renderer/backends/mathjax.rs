use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::{DiagramBlock, mathjax_renderer::MathJaxRendererOps};
use crate::renderer::api::{DiagramKind, RenderError, RenderInput, RenderOutput, Renderer};
use crate::renderer::output::RenderOutputFactory;
use crate::renderer::runtime::RuntimeDescriptor;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MathJaxRenderer {
    runtime_path: PathBuf,
}

impl MathJaxRenderer {
    pub fn with_runtime_path(runtime_path: PathBuf) -> Self {
        Self { runtime_path }
    }
}

impl Renderer for MathJaxRenderer {
    fn render(&self, input: &RenderInput) -> Result<RenderOutput, RenderError> {
        if input.kind != DiagramKind::MathJax {
            return Err(RenderError::UnsupportedKind);
        }
        let runtime = RuntimeDescriptor::mathjax();
        match self.render_block(input) {
            crate::markdown::DiagramResult::Ok(svg) => RenderOutputFactory::from_diagram_result(
                input,
                crate::markdown::DiagramResult::Ok(svg),
                runtime,
            ),
            crate::markdown::DiagramResult::RawCode { source, warning } => Ok(
                RenderOutputFactory::from_raw_string(input, source, warning, runtime),
            ),
            crate::markdown::DiagramResult::Err { error, source } => Ok(
                RenderOutputFactory::from_raw_string(input, source, error, runtime),
            ),
            _ => Err(RenderError::Runtime(
                "unexpected MathJax output".to_string(),
            )),
        }
    }
}

impl MathJaxRenderer {
    fn render_block(&self, input: &RenderInput) -> crate::markdown::DiagramResult {
        let block = DiagramBlock {
            kind: crate::markdown::DiagramKind::MathJax,
            source: input.source.clone(),
        };
        let preset = DiagramColorPreset::for_render_input(input);
        MathJaxRendererOps::render_mathjax_with_runtime_path(
            &block,
            &self.runtime_path,
            &preset,
            MathJaxRenderConfig::display(input),
        )
    }
}

struct MathJaxRenderConfig;

impl MathJaxRenderConfig {
    fn display(input: &RenderInput) -> bool {
        input
            .config
            .vendor_config
            .get("display")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    }
}
