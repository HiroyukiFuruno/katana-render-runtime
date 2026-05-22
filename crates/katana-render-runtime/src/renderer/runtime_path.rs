use super::api::{DiagramKind, RenderError};
use crate::markdown::{
    drawio_renderer::DrawioRendererOps, mathjax_renderer::MathJaxRendererOps,
    mermaid_renderer::MermaidBinaryOps, plantuml_renderer::PlantUmlRendererOps,
};
use std::path::PathBuf;

pub struct RuntimePathResolver;

impl RuntimePathResolver {
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
        if std::env::var_os("KDR_PLANTUML_JAR").is_some()
            || std::env::var_os("PLANTUML_JAR").is_some()
        {
            return Ok(());
        }
        let resolved = RuntimePathResolver::resolve_with_plantuml_cache_dir(
            DiagramKind::PlantUml,
            None,
            Some(PathBuf::from("/tmp/kdr-api-cache")),
        )?;

        assert_eq!(
            resolved,
            PathBuf::from("/tmp/kdr-api-cache")
                .join(PLANTUML_JAR_VERSION)
                .join("plantuml.jar")
        );
        Ok(())
    }
}
