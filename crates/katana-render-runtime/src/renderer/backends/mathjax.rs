use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::runtime_resource_loader::{
    BundledResource, RuntimeResourceLoader as runtime_resource_loader,
};
use crate::markdown::{DiagramBlock, mathjax_renderer::MathJaxRendererOps};
use crate::renderer::api::{DiagramKind, RenderError, RenderInput, RenderOutput, Renderer};
use crate::renderer::output::RenderOutputFactory;
use crate::renderer::runtime::RuntimeDescriptor;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MathJaxRenderer {
    runtime_path: PathBuf,
    bundled: bool,
}

impl MathJaxRenderer {
    pub fn with_runtime_path(runtime_path: PathBuf) -> Self {
        Self {
            runtime_path,
            bundled: false,
        }
    }
    pub(crate) fn with_bundled_runtime_path(runtime_path: PathBuf) -> Self {
        Self {
            runtime_path,
            bundled: true,
        }
    }
}

impl Renderer for MathJaxRenderer {
    fn render(&self, input: &RenderInput) -> Result<RenderOutput, RenderError> {
        if input.kind != DiagramKind::MathJax {
            return Err(RenderError::UnsupportedKind);
        }
        let runtime = RuntimeDescriptor::mathjax();
        let (result, digest) = self.render_block(input);
        match result {
            Ok(svg) => RenderOutputFactory::from_diagram_result(
                input,
                crate::markdown::DiagramResult::Ok(svg),
                runtime,
            )
            .map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref())),
            Err(warning) => {
                let error_key = format!("error:{}:{warning}", digest.as_deref().unwrap_or(""));
                Ok(RenderOutputFactory::bind_resource(
                    RenderOutputFactory::from_raw_string(
                        input,
                        input.source.clone(),
                        warning,
                        runtime,
                    ),
                    Some(&error_key),
                ))
            }
        }
    }
}

impl MathJaxRenderer {
    fn render_block(&self, input: &RenderInput) -> (Result<String, String>, Option<String>) {
        let block = DiagramBlock {
            kind: crate::markdown::DiagramKind::MathJax,
            source: input.source.clone(),
        };
        if block.source.trim().is_empty() {
            return (Err("MathJax source is empty".to_string()), None);
        }
        let resource = match self.read_resource() {
            Ok(resource) => resource,
            Err(error) => return (Err(error), None),
        };
        let preset = DiagramColorPreset::for_render_input(input);
        let result = MathJaxRendererOps::render_mathjax_with_loaded(
            &block,
            &preset,
            MathJaxRenderConfig::display(input),
            &resource,
        );
        (result, resource.into_digest())
    }
    fn read_resource(
        &self,
    ) -> Result<crate::markdown::runtime_resource_loader::LoadedResource, String> {
        runtime_resource_loader::load(
            &self.runtime_path,
            self.bundled.then_some(BundledResource::MathJax),
        )
        .map_err(|error| self.resource_error(error))
    }

    fn resource_error(&self, error: std::io::Error) -> String {
        if error.kind() == std::io::ErrorKind::NotFound {
            return format!(
                "MathJax runtime asset is not installed: {}",
                self.runtime_path.display()
            );
        }
        format!(
            "MathJax runtime asset could not be read: {}: {error}",
            self.runtime_path.display()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::api::{RenderConfig, RenderContext, RenderPolicy};

    #[test]
    fn display_config_accepts_only_boolean_values() {
        assert!(MathJaxRenderConfig::display(&input(serde_json::json!({
            "display": true
        }))));
        assert!(!MathJaxRenderConfig::display(&input(serde_json::json!({
            "display": "true"
        }))));
        assert!(!MathJaxRenderConfig::display(&input(serde_json::json!({}))));
    }

    fn input(vendor_config: serde_json::Value) -> RenderInput {
        RenderInput {
            kind: DiagramKind::MathJax,
            source: String::new(),
            config: RenderConfig { vendor_config },
            policy: RenderPolicy::default(),
            context: RenderContext::default(),
        }
    }
}
