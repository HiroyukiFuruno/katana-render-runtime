use super::api::{DiagramKind, RenderError, Renderer};
use super::backends::{DrawioRenderer, MathJaxRenderer, MermaidRenderer, PlantUmlRenderer};
use crate::markdown::{
    drawio_renderer::DrawioRendererOps, mathjax_renderer::MathJaxRendererOps,
    mermaid_renderer::MermaidBinaryOps, plantuml_renderer::PlantUmlRendererOps,
};
use std::path::PathBuf;

type RendererWithPath = (PathBuf, Box<dyn Renderer>);

pub struct RuntimePathResolver;

impl RuntimePathResolver {
    pub fn resolve_renderer(
        kind: DiagramKind,
        runtime_path: Option<PathBuf>,
        plantuml_cache_dir: Option<PathBuf>,
    ) -> Result<RendererWithPath, RenderError> {
        let env_name = Self::runtime_env_name(kind);
        let env_value = if runtime_path.is_none() {
            env_name.and_then(std::env::var_os)
        } else {
            None
        };
        let bundled = runtime_path.is_none() && env_name.is_some() && env_value.is_none();
        let path = match runtime_path {
            Some(path) => path,
            None => Self::resolve_selected_path(kind, env_value, plantuml_cache_dir)?,
        };
        let renderer = Self::make_renderer(kind, &path, bundled);
        Ok((path, renderer))
    }

    fn runtime_env_name(kind: DiagramKind) -> Option<&'static str> {
        match kind {
            DiagramKind::Mermaid => Some("MERMAID_JS"),
            DiagramKind::Drawio => Some("DRAWIO_JS"),
            DiagramKind::MathJax => Some("MATHJAX_JS"),
            DiagramKind::PlantUml => None,
        }
    }

    fn resolve_selected_path(
        kind: DiagramKind,
        env_value: Option<std::ffi::OsString>,
        plantuml_cache_dir: Option<PathBuf>,
    ) -> Result<PathBuf, RenderError> {
        match kind {
            DiagramKind::Mermaid => MermaidBinaryOps::resolve_mermaid_js_with_env(
                env_value,
                MermaidBinaryOps::default_install_path(),
            ),
            DiagramKind::Drawio => DrawioRendererOps::resolve_drawio_js_with_env(
                env_value,
                DrawioRendererOps::default_install_path(),
            ),
            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js_with_env(
                env_value,
                MathJaxRendererOps::default_install_path(),
            ),
            DiagramKind::PlantUml => Ok(plantuml_cache_dir
                .map_or_else(PlantUmlRendererOps::default_jar_path, |path| {
                    PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)
                })),
        }
        .map_err(RenderError::RuntimeResolution)
    }

    fn make_renderer(
        kind: DiagramKind,
        path: &std::path::Path,
        bundled: bool,
    ) -> Box<dyn Renderer> {
        let path = path.to_path_buf();
        match kind {
            DiagramKind::Mermaid => Box::new(if bundled {
                MermaidRenderer::with_bundled_runtime_path(path)
            } else {
                MermaidRenderer::with_runtime_path(path)
            }),
            DiagramKind::Drawio => Box::new(if bundled {
                DrawioRenderer::with_bundled_runtime_path(path)
            } else {
                DrawioRenderer::with_runtime_path(path)
            }),
            DiagramKind::MathJax => Box::new(if bundled {
                MathJaxRenderer::with_bundled_runtime_path(path)
            } else {
                MathJaxRenderer::with_runtime_path(path)
            }),
            DiagramKind::PlantUml => Box::new(PlantUmlRenderer::with_runtime_path(path)),
        }
    }

    pub fn resolve(
        kind: DiagramKind,
        runtime_path: Option<PathBuf>,
    ) -> Result<PathBuf, RenderError> {
        Self::resolve_with_plantuml_cache_dir(kind, runtime_path, None)
    }

    pub fn resolve_with_plantuml_cache_dir(
        kind: DiagramKind,
        runtime_path: Option<PathBuf>,
        plantuml_cache_dir: Option<PathBuf>,
    ) -> Result<PathBuf, RenderError> {
        if let Some(path) = runtime_path {
            return Ok(path);
        }
        match kind {
            DiagramKind::Mermaid => MermaidBinaryOps::resolve_mermaid_js(),
            DiagramKind::Drawio => DrawioRendererOps::resolve_drawio_js(),
            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),
            DiagramKind::PlantUml => Ok(plantuml_cache_dir
                .map_or_else(PlantUmlRendererOps::default_jar_path, |path| {
                    PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)
                })),
        }
        .map_err(RenderError::RuntimeResolution)
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimePathResolver;
    use crate::markdown::plantuml_renderer::PLANTUML_JAR_VERSION;
    use crate::renderer::api::DiagramKind;
    use std::path::PathBuf;

    #[test]
    fn explicit_runtime_path_wins_at_surface_boundary() -> Result<(), Box<dyn std::error::Error>> {
        let path = PathBuf::from("/tmp/runtime.js");
        let resolved = RuntimePathResolver::resolve(DiagramKind::Mermaid, Some(path.clone()))?;

        assert_eq!(resolved, path);
        Ok(())
    }

    #[test]
    fn plantuml_cache_dir_can_be_overridden_at_api_boundary()
    -> Result<(), Box<dyn std::error::Error>> {
        let kind = DiagramKind::PlantUml;
        let cache_dir = PathBuf::from("/tmp/krr-api-cache");
        let resolved =
            RuntimePathResolver::resolve_with_plantuml_cache_dir(kind, None, Some(cache_dir))?;

        assert!(resolved.to_string_lossy().contains(PLANTUML_JAR_VERSION));
        assert!(resolved.ends_with("plantuml.jar"));
        Ok(())
    }

    #[test]
    fn plantuml_default_runtime_path_is_resolvable() -> Result<(), Box<dyn std::error::Error>> {
        let resolved = RuntimePathResolver::resolve(DiagramKind::PlantUml, None)?;

        assert!(resolved.ends_with("plantuml.jar"));
        Ok(())
    }
}
