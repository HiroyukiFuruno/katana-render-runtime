use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::runtime_resource_loader::{
    BundledResource, RuntimeResourceLoader as runtime_resource_loader,
};
use crate::markdown::{DiagramBlock, mermaid_renderer::MermaidRenderOps};
use crate::renderer::api::{DiagramKind, RenderError, RenderInput, RenderOutput, Renderer};
use crate::renderer::output::RenderOutputFactory;
use crate::renderer::runtime::RuntimeDescriptor;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MermaidRenderer {
    runtime_path: PathBuf,
    bundled: bool,
}

impl MermaidRenderer {
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

impl Renderer for MermaidRenderer {
    fn render(&self, input: &RenderInput) -> Result<RenderOutput, RenderError> {
        if input.kind != DiagramKind::Mermaid {
            return Err(RenderError::UnsupportedKind);
        }
        let (result, digest) = self.render_block(input);
        RenderOutputFactory::from_diagram_result(input, result, RuntimeDescriptor::mermaid())
            .map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))
    }
}

impl MermaidRenderer {
    fn render_block(
        &self,
        input: &RenderInput,
    ) -> (crate::markdown::DiagramResult, Option<String>) {
        let block = DiagramBlock {
            kind: crate::markdown::DiagramKind::Mermaid,
            source: input.source.clone(),
        };
        let preset = DiagramColorPreset::for_render_input(input);
        if block.source.trim().is_empty() {
            return (self.render_empty(&block, &preset), None);
        }
        let resource = match self.read_resource(&block) {
            Ok(resource) => resource,
            Err(error) => return (error, None),
        };
        let result = MermaidRenderOps::render_mermaid_with_loaded(
            &block,
            &self.runtime_path,
            &preset,
            &resource,
        );
        (result, resource.into_digest())
    }

    fn render_empty(
        &self,
        block: &DiagramBlock,
        preset: &DiagramColorPreset,
    ) -> crate::markdown::DiagramResult {
        MermaidRenderOps::render_mermaid_with_origin(
            block,
            &self.runtime_path,
            preset,
            self.bundled,
        )
    }

    fn read_resource(
        &self,
        block: &DiagramBlock,
    ) -> Result<
        crate::markdown::runtime_resource_loader::LoadedResource,
        crate::markdown::DiagramResult,
    > {
        runtime_resource_loader::load(
            &self.runtime_path,
            self.bundled.then_some(BundledResource::Mermaid),
        )
        .map_err(|error| {
            runtime_resource_loader::diagram_error(
                block,
                &self.runtime_path,
                error,
                "Mermaid",
                crate::markdown::runtime_assets::MERMAID_DOWNLOAD_URL,
                "Failed to read Mermaid.js bundle",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::api::{RenderConfig, RenderContext, RenderPolicy};

    #[test]
    fn resource_fingerprint_stays_stable_across_svg_instances_and_tracks_changes()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("runtime.js");
        let bundle = r#"globalThis.mermaid = { initialize() {}, render: async (id) => ({svg: `<svg xmlns="http://www.w3.org/2000/svg" id="${id}" width="20" height="10" viewBox="0 0 20 10"></svg>`}) };"#;
        std::fs::write(&path, bundle)?;
        let renderer = MermaidRenderer::with_runtime_path(path.clone());
        let input = test_input();
        let first = renderer.render(&input)?;
        let second = renderer.render(&input)?;
        assert_ne!(first.svg, second.svg);
        assert_eq!(first.cache_fingerprint, second.cache_fingerprint);
        std::fs::write(&path, format!("{bundle}\n// changed runtime"))?;
        let changed = renderer.render(&input)?;
        assert_ne!(first.cache_fingerprint, changed.cache_fingerprint);
        std::fs::write(&path, [0xff])?;
        assert!(renderer.render(&input).is_err());
        std::fs::remove_file(&path)?;
        assert!(renderer.render(&input).is_err());
        Ok(())
    }
    fn test_input() -> RenderInput {
        RenderInput {
            kind: DiagramKind::Mermaid,
            source: format!("graph TD; A{}-->B", std::process::id()),
            config: RenderConfig::default(),
            policy: RenderPolicy::default(),
            context: RenderContext::default(),
        }
    }
}
