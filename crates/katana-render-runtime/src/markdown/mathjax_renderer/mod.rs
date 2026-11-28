mod js_runtime;
mod js_runtime_scripts;
pub mod types;

use crate::markdown::DiagramBlock;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::runtime_assets::RuntimeAsset;
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

    pub(crate) fn resolve_mathjax_js_with_env(
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

    #[cfg(test)]
    pub(crate) fn render_mathjax_with_runtime_path(
        block: &DiagramBlock,
        runtime_path: &Path,
        preset: &DiagramColorPreset,
        display: bool,
    ) -> Result<String, String> {
        Self::render_mathjax_with_origin(block, runtime_path, preset, display, false)
    }

    #[cfg(test)]
    pub(crate) fn render_mathjax_with_origin(
        block: &DiagramBlock,
        runtime_path: &Path,
        preset: &DiagramColorPreset,
        display: bool,
        bundled: bool,
    ) -> Result<String, String> {
        if block.source.trim().is_empty() {
            return Err("MathJax source is empty".to_string());
        }
        MathJaxJsRuntimeOps::render_with_origin(
            &block.source,
            runtime_path,
            preset,
            display,
            bundled,
        )
    }
    pub(crate) fn render_mathjax_with_loaded(
        block: &DiagramBlock,
        preset: &DiagramColorPreset,
        display: bool,
        resource: &crate::markdown::runtime_resource_loader::LoadedResource,
    ) -> Result<String, String> {
        if block.source.trim().is_empty() {
            return Err("MathJax source is empty".to_string());
        }
        MathJaxJsRuntimeOps::render_bundle(&block.source, resource.text(), preset, display)
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
        RuntimeAsset::exists_with_matching_bytes(path, |existing| {
            Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes())
        })
    }
}

fn runtime_asset_error(error: std::io::Error) -> String {
    error.to_string()
}

#[cfg(all(test, unix))]
#[path = "generated_runtime_boundary_tests.rs"]
mod generated_runtime_boundary_tests;

#[cfg(test)]
#[path = "mathjax_renderer_tests.rs"]
mod tests;
