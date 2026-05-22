use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::{
    DiagramBlock,
    plantuml_renderer::{
        PlantUmlRendererOps, PlantUmlRuntimeConfig, PlantUmlThemeCatalog, PlantUmlThemeConfig,
    },
};
use crate::renderer::api::{DiagramKind, RenderError, RenderInput, RenderOutput, Renderer};
use crate::renderer::output::RenderOutputFactory;
use crate::renderer::runtime::RuntimeDescriptor;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PlantUmlRenderer {
    jar_path: PathBuf,
}

impl PlantUmlRenderer {
    pub fn with_runtime_path(jar_path: PathBuf) -> Self {
        Self { jar_path }
    }

    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self {
            jar_path: PlantUmlRendererOps::default_jar_path_for_cache_dir(&cache_dir),
        }
    }

    pub fn available_themes() -> &'static [&'static str] {
        PlantUmlThemeCatalog::names()
    }
}

impl Renderer for PlantUmlRenderer {
    fn render(&self, input: &RenderInput) -> Result<RenderOutput, RenderError> {
        if input.kind != DiagramKind::PlantUml {
            return Err(RenderError::UnsupportedKind);
        }
        RenderOutputFactory::from_diagram_result(
            input,
            self.render_block(input),
            RuntimeDescriptor::plantuml(),
        )
    }
}

impl PlantUmlRenderer {
    fn render_block(&self, input: &RenderInput) -> crate::markdown::DiagramResult {
        let block = DiagramBlock {
            kind: crate::markdown::DiagramKind::PlantUml,
            source: input.source.clone(),
        };
        let config = match PlantUmlRenderConfig::from_input(input) {
            Ok(config) => config,
            Err(error) => {
                return crate::markdown::DiagramResult::Err {
                    source: input.source.clone(),
                    error: format!("invalid PlantUML config: {error}"),
                };
            }
        };
        let preset = DiagramColorPreset::for_render_input(input);
        PlantUmlRendererOps::render_plantuml_with_jar_path(
            &block,
            &self.jar_path,
            &preset,
            &config.theme,
            &config.runtime,
        )
    }
}

struct PlantUmlRenderConfig {
    theme: PlantUmlThemeConfig,
    runtime: PlantUmlRuntimeConfig,
}

impl PlantUmlRenderConfig {
    fn from_input(input: &RenderInput) -> Result<Self, String> {
        Ok(Self {
            theme: PlantUmlThemeConfig::from_value(&input.config.vendor_config)?,
            runtime: PlantUmlRuntimeConfig::from_value(&input.config.vendor_config)?,
        })
    }
}
