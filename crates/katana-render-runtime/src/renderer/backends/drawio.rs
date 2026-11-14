use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::runtime_resource_loader::{
    BundledResource, RuntimeResourceLoader as runtime_resource_loader,
};
use crate::markdown::{DiagramBlock, drawio_renderer::DrawioRendererOps};
use crate::renderer::api::{DiagramKind, RenderError, RenderInput, RenderOutput, Renderer};
use crate::renderer::output::RenderOutputFactory;
use crate::renderer::runtime::RuntimeDescriptor;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DrawioRenderer {
    runtime_path: PathBuf,
    bundled: bool,
}

impl DrawioRenderer {
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

impl Renderer for DrawioRenderer {
    fn render(&self, input: &RenderInput) -> Result<RenderOutput, RenderError> {
        if input.kind != DiagramKind::Drawio {
            return Err(RenderError::UnsupportedKind);
        }
        let (result, digest) = self.render_block(input);
        RenderOutputFactory::from_diagram_result(input, result, RuntimeDescriptor::drawio())
            .map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))
    }
}

impl DrawioRenderer {
    fn render_block(
        &self,
        input: &RenderInput,
    ) -> (crate::markdown::DiagramResult, Option<String>) {
        let block = DiagramBlock {
            kind: crate::markdown::DiagramKind::DrawIo,
            source: input.source.clone(),
        };
        let preset = DiagramColorPreset::for_render_input(input);
        let resource = match self.read_resource(&block) {
            Ok(resource) => resource,
            Err(error) => return (error, None),
        };
        let result = DrawioRendererOps::render_drawio_with_loaded(&block, &preset, &resource);
        (result, resource.into_digest())
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
            self.bundled.then_some(BundledResource::Drawio),
        )
        .map_err(|error| {
            runtime_resource_loader::diagram_error(
                block,
                &self.runtime_path,
                error,
                "Draw.io",
                crate::markdown::runtime_assets::DRAWIO_DOWNLOAD_URL,
                "Failed to read Draw.io JavaScript bundle",
            )
        })
    }
}
