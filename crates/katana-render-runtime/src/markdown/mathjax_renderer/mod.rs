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

#[cfg(test)]
mod tests {
    use super::{MathJaxGeneratedRuntimeAsset, MathJaxRendererOps};
    use crate::markdown::DiagramBlock;
    use crate::markdown::DiagramKind;
    use crate::markdown::color_preset::DiagramColorPreset;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_ID: AtomicUsize = AtomicUsize::new(0);
    type MaterializeResult = Result<PathBuf, String>;
    type MaterializeLifecycle = (MaterializeResult, MaterializeResult, MaterializeResult);

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

    #[test]
    fn resolve_mathjax_js_rejects_empty_env_and_missing_bundled_path() {
        let empty =
            MathJaxRendererOps::resolve_mathjax_js_with_env(Some(std::ffi::OsString::new()), None);
        let missing = MathJaxRendererOps::resolve_mathjax_js_with_env(None, None);

        assert!(matches!(empty, Err(error) if error.contains("MATHJAX_JS is empty")));
        assert!(
            matches!(missing, Err(error) if error.contains("bundled MathJax path is unavailable"))
        );
    }

    #[test]
    fn generated_runtime_materializes_reuses_and_replaces_files() {
        let path = test_path("lifecycle.js");
        let (first, second, third) = materialize_runtime_lifecycle(&path);

        assert_materialized(first, &path);
        assert_materialized(second, &path);
        assert_materialized(third, &path);
        assert_runtime_file_replaced(&path);
    }

    #[test]
    fn generated_runtime_reports_invalid_paths_and_empty_source() {
        let empty = MathJaxGeneratedRuntimeAsset::materialize_at(PathBuf::new());
        let directory = test_path("directory");
        assert!(std::fs::create_dir_all(&directory).is_ok());
        let read_error = MathJaxGeneratedRuntimeAsset::materialize_at(directory);
        let block = DiagramBlock {
            kind: DiagramKind::MathJax,
            source: " ".to_string(),
        };
        let rendered = MathJaxRendererOps::render_mathjax_with_runtime_path(
            &block,
            std::path::Path::new("missing.js"),
            DiagramColorPreset::current(),
            false,
        );

        assert!(matches!(empty, Err(error) if error.contains("has no parent")));
        assert!(read_error.is_err());
        assert!(matches!(rendered, Err(error) if error.contains("source is empty")));
    }

    #[test]
    fn render_mathjax_with_runtime_path_returns_runtime_svg() {
        let runtime = test_path("runtime-success.js");
        assert!(
            std::fs::write(
                &runtime,
                r#"function katanaRunMathJaxRuntime() { return '{"kind":"svg","svg":"<svg/>"}'; }"#,
            )
            .is_ok()
        );
        let block = DiagramBlock {
            kind: DiagramKind::MathJax,
            source: "x".to_string(),
        };

        let rendered = MathJaxRendererOps::render_mathjax_with_runtime_path(
            &block,
            &runtime,
            DiagramColorPreset::current(),
            true,
        );

        assert!(matches!(rendered, Ok(value) if value == "<svg/>"));
    }

    fn test_path(name: &str) -> PathBuf {
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "krr-mathjax-generated-{name}-{}-{id}",
            std::process::id()
        ))
    }

    fn materialize_runtime_lifecycle(path: &Path) -> MaterializeLifecycle {
        let first = MathJaxGeneratedRuntimeAsset::materialize_at(path.to_path_buf());
        let second = MathJaxGeneratedRuntimeAsset::materialize_at(path.to_path_buf());
        assert!(std::fs::write(path, b"stale").is_ok());
        let third = MathJaxGeneratedRuntimeAsset::materialize_at(path.to_path_buf());
        (first, second, third)
    }

    fn assert_materialized(result: Result<PathBuf, String>, path: &Path) {
        assert!(matches!(result, Ok(value) if value == path));
    }

    fn assert_runtime_file_replaced(path: &Path) {
        assert!(matches!(std::fs::read(path), Ok(bytes) if bytes != b"stale"));
    }
}
