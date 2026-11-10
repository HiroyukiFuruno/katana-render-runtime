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
    assert!(matches!(missing, Err(error) if error.contains("bundled MathJax path is unavailable")));
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

#[test]
fn loaded_mathjax_bundle_rejects_blank_input_and_renders_real_math() -> Result<(), String> {
    use crate::markdown::runtime_resource_loader::{BundledResource, RuntimeResourceLoader};

    let resource =
        RuntimeResourceLoader::load(Path::new("unused.js"), Some(BundledResource::MathJax))
            .map_err(|error| error.to_string())?;
    let preset = DiagramColorPreset::current();
    let mut block = DiagramBlock {
        kind: DiagramKind::MathJax,
        source: " \t\n".into(),
    };
    assert_eq!(
        MathJaxRendererOps::render_mathjax_with_loaded(&block, preset, false, &resource),
        Err("MathJax source is empty".into())
    );
    block.source = "x^2".into();
    let rendered =
        MathJaxRendererOps::render_mathjax_with_loaded(&block, preset, false, &resource)?;
    assert!(rendered.contains("<svg"));
    Ok(())
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
