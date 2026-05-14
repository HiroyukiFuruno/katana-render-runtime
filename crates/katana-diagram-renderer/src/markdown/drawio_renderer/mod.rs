mod js_runtime;
mod js_runtime_resources;
mod js_runtime_scripts;
pub mod types;

pub use crate::markdown::runtime_assets::{DRAWIO_JS_CHECKSUM, DRAWIO_JS_VERSION};
pub use types::DrawioRendererOps;

use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::runtime_assets::{DRAWIO_DOWNLOAD_URL, RuntimeAsset};
use crate::markdown::{DiagramBlock, DiagramResult};
use js_runtime::DrawioJsRuntimeOps;
use std::path::PathBuf;

impl DrawioRendererOps {
    pub fn default_install_path() -> Option<PathBuf> {
        Some(RuntimeAsset::drawio().materialized_path())
    }

    pub fn resolve_drawio_js() -> Result<PathBuf, String> {
        Self::resolve_drawio_js_with_env(
            std::env::var_os("DRAWIO_JS"),
            Self::default_install_path(),
        )
    }

    fn resolve_drawio_js_with_env(
        env_value: Option<std::ffi::OsString>,
        home_path: Option<PathBuf>,
    ) -> Result<PathBuf, String> {
        if let Some(path) = Self::env_drawio_js_from(env_value)? {
            return Ok(path);
        }

        Self::resolve_drawio_js_with_home(home_path)
    }

    fn env_drawio_js_from(value: Option<std::ffi::OsString>) -> Result<Option<PathBuf>, String> {
        let Some(path) = value else {
            return Ok(None);
        };
        if path.is_empty() {
            return Err("DRAWIO_JS is empty".to_string());
        }
        Ok(Some(PathBuf::from(path)))
    }

    fn resolve_drawio_js_with_home(home_path: Option<PathBuf>) -> Result<PathBuf, String> {
        let Some(path) = home_path else {
            return Err("bundled Draw.io runtime path is unavailable".to_string());
        };
        RuntimeAsset::drawio().materialize_at(path)
    }

    pub fn find_drawio_js() -> Result<Option<PathBuf>, String> {
        Self::find_drawio_js_from(Self::resolve_drawio_js())
    }

    fn find_drawio_js_from(path: Result<PathBuf, String>) -> Result<Option<PathBuf>, String> {
        let path = path?;
        Ok(path.exists().then_some(path))
    }

    pub fn render_drawio_with_runtime_path(
        block: &DiagramBlock,
        drawio_js: &std::path::Path,
        preset: &DiagramColorPreset,
    ) -> DiagramResult {
        if !drawio_js.exists() {
            return DiagramResult::NotInstalled {
                kind: "Draw.io".to_string(),
                download_url: DRAWIO_DOWNLOAD_URL.to_string(),
                install_path: drawio_js.to_path_buf(),
            };
        }

        match DrawioJsRuntimeOps::render(&block.source, drawio_js, preset) {
            Ok(svg) => DiagramResult::Ok(svg),
            Err(error) => DiagramResult::Err {
                source: block.source.clone(),
                error,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DrawioRendererOps;
    use crate::markdown::{DiagramBlock, DiagramKind, DiagramResult};

    #[test]
    fn render_with_missing_runtime_reports_install_path() {
        let block = DiagramBlock {
            kind: DiagramKind::DrawIo,
            source: "<mxGraphModel><root /></mxGraphModel>".to_string(),
        };
        let result = DrawioRendererOps::render_drawio_with_runtime_path(
            &block,
            std::path::Path::new("target/kdr-tests/missing-drawio.min.js"),
            crate::markdown::color_preset::DiagramColorPreset::current(),
        );
        assert!(matches!(result, DiagramResult::NotInstalled { .. }));
    }

    #[test]
    fn resolve_drawio_js_reports_missing_default_without_fallback() {
        let result = DrawioRendererOps::resolve_drawio_js_with_home(None);

        assert!(matches!(result, Err(error) if error.contains("bundled Draw.io")));
    }

    #[test]
    fn resolve_drawio_js_uses_versioned_repository_asset_without_env() {
        let result = DrawioRendererOps::resolve_drawio_js_with_env(
            None,
            DrawioRendererOps::default_install_path(),
        );

        assert!(matches!(
            result,
            Ok(path) if path.ends_with("vendor/drawio/29.7.10/drawio.min.js")
        ));
    }

    #[test]
    fn resolve_drawio_js_accepts_explicit_env_override() {
        let result = DrawioRendererOps::resolve_drawio_js_with_env(
            Some(std::ffi::OsString::from("runtime.js")),
            None,
        );

        assert!(matches!(result, Ok(path) if path == std::path::Path::new("runtime.js")));
    }

    #[test]
    fn resolve_drawio_js_rejects_empty_env_override() {
        let result = DrawioRendererOps::resolve_drawio_js_with_env(
            Some(std::ffi::OsString::new()),
            Some(std::path::PathBuf::from("fallback.js")),
        );

        assert!(matches!(result, Err(error) if error.contains("DRAWIO_JS")));
    }

    #[test]
    fn find_drawio_js_propagates_resolution_errors() {
        let result = DrawioRendererOps::find_drawio_js_from(Err("boom".to_string()));

        assert!(matches!(result, Err(error) if error == "boom"));
    }

    #[test]
    fn env_drawio_js_rejects_empty_override() {
        let result = DrawioRendererOps::env_drawio_js_from(Some(std::ffi::OsString::new()));

        assert!(matches!(result, Err(error) if error.contains("DRAWIO_JS")));
    }

    #[test]
    fn env_drawio_js_treats_missing_override_as_optional() {
        let result = DrawioRendererOps::env_drawio_js_from(None);

        assert!(matches!(result, Ok(None)));
    }
}
