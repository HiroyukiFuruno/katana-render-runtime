use super::api::{DiagramKind, RenderError};
use crate::markdown::{drawio_renderer::DrawioRendererOps, mermaid_renderer::MermaidBinaryOps};
use std::path::PathBuf;

pub struct RuntimePathResolver;

impl RuntimePathResolver {
    pub fn resolve(
        kind: DiagramKind,
        runtime_path: Option<PathBuf>,
    ) -> Result<PathBuf, RenderError> {
        if let Some(path) = runtime_path {
            return Ok(path);
        }
        match kind {
            DiagramKind::Mermaid => MermaidBinaryOps::resolve_mermaid_js(),
            DiagramKind::Drawio => DrawioRendererOps::resolve_drawio_js(),
        }
        .map_err(RenderError::RuntimeResolution)
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimePathResolver;
    use crate::renderer::api::DiagramKind;
    use std::path::PathBuf;

    #[test]
    fn explicit_runtime_path_wins_at_surface_boundary() -> Result<(), Box<dyn std::error::Error>> {
        let path = PathBuf::from("/tmp/runtime.js");
        let resolved = RuntimePathResolver::resolve(DiagramKind::Mermaid, Some(path.clone()))?;

        assert_eq!(resolved, path);
        Ok(())
    }
}
