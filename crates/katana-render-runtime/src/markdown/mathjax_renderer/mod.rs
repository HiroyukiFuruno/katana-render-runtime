mod js_runtime;
mod js_runtime_scripts;
pub mod types;

use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::{DiagramBlock, DiagramResult};
use js_runtime::MathJaxJsRuntimeOps;
use js_runtime_scripts::MathJaxRuntimeScripts;
use std::path::{Path, PathBuf};
pub use types::MathJaxRendererOps;

pub use crate::markdown::runtime_assets::{
    MATHJAX_DOWNLOAD_URL as MATHJAX_RUNTIME_DOWNLOAD_URL,
    MATHJAX_JS_CHECKSUM as MATHJAX_RUNTIME_CHECKSUM, MATHJAX_JS_VERSION as MATHJAX_RUNTIME_VERSION,
};

impl MathJaxRendererOps {
    pub fn default_install_path() -> Option<PathBuf> {
        Some(
            std::env::temp_dir()
                .join("katana-render-runtime")
                .join("generated")
                .join("mathjax-runtime.min.js"),
        )
    }

    pub fn resolve_mathjax_js() -> Result<PathBuf, String> {
        Self::resolve_mathjax_js_with_env(
            std::env::var_os("MATHJAX_JS"),
            Self::default_install_path(),
        )
    }

    fn resolve_mathjax_js_with_env(
        env_value: Option<std::ffi::OsString>,
        bundled_path: Option<PathBuf>,
    ) -> Result<PathBuf, String> {
        if let Some(path) = Self::env_mathjax_js_from(env_value)? {
            return Ok(path);
        }
        let Some(path) = bundled_path else {
            return Err("bundled MathJax path is unavailable".to_string());
        };
        MathJaxGeneratedRuntimeAsset::materialize_at(path)
    }

    fn env_mathjax_js_from(value: Option<std::ffi::OsString>) -> Result<Option<PathBuf>, String> {
        let Some(path) = value else {
            return Ok(None);
        };
        if path.is_empty() {
            return Err("MATHJAX_JS is empty".to_string());
        }
        Ok(Some(PathBuf::from(path)))
    }

    pub(crate) fn render_mathjax_with_runtime_path(
        block: &DiagramBlock,
        runtime_path: &Path,
        preset: &DiagramColorPreset,
        display: bool,
    ) -> DiagramResult {
        if block.source.trim().is_empty() {
            return Self::raw(block, "MathJax source is empty".to_string());
        }
        MathJaxJsRuntimeOps::render(&block.source, runtime_path, preset, display)
            .map_or_else(|error| Self::raw(block, error), DiagramResult::Ok)
    }

    fn raw(block: &DiagramBlock, warning: String) -> DiagramResult {
        DiagramResult::RawCode {
            source: block.source.clone(),
            warning,
        }
    }
}

struct MathJaxGeneratedRuntimeAsset;

impl MathJaxGeneratedRuntimeAsset {
    fn materialize_at(path: PathBuf) -> Result<PathBuf, String> {
        if Self::exists_with_same_bytes(&path)? {
            return Ok(path);
        }
        let Some(parent) = path.parent() else {
            return Err("MathJax generated runtime path has no parent".to_string());
        };
        std::fs::create_dir_all(parent).map_err(runtime_asset_error)?;
        std::fs::write(&path, MathJaxRuntimeScripts::runtime_source())
            .map_err(runtime_asset_error)?;
        Ok(path)
    }

    fn exists_with_same_bytes(path: &Path) -> Result<bool, String> {
        match std::fs::read(path) {
            Ok(existing) => Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(runtime_asset_error(error)),
        }
    }
}

fn runtime_asset_error(error: std::io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::MathJaxRendererOps;

    #[test]
    fn resolve_mathjax_js_uses_versioned_repository_asset_without_env() {
        let result = MathJaxRendererOps::resolve_mathjax_js_with_env(
            None,
            MathJaxRendererOps::default_install_path(),
        );

        assert!(matches!(
            result,
            Ok(path) if path.ends_with("generated/mathjax-runtime.min.js")
        ));
    }

    #[test]
    fn resolve_mathjax_js_accepts_explicit_env_override() {
        let result = MathJaxRendererOps::resolve_mathjax_js_with_env(
            Some(std::ffi::OsString::from("runtime.js")),
            None,
        );

        assert!(matches!(result, Ok(path) if path == std::path::Path::new("runtime.js")));
    }
}
