use katana_render_runtime::markdown::color_preset::DiagramColorPreset;
use katana_render_runtime::markdown::drawio_renderer::{DRAWIO_JS_CHECKSUM, DrawioRendererOps};
use katana_render_runtime::markdown::{DiagramBlock, DiagramKind, DiagramResult};
use std::path::{Path, PathBuf};
#[path = "support/katana_transfer.rs"]
mod katana_transfer;
use katana_transfer::{TestResult, env_guard, missing_runtime_path, test_error};
#[path = "support/katana_env_override.rs"]
mod katana_env_override;
use katana_env_override::EnvOverride;
#[path = "support/drawio_fixture.rs"]
mod drawio_fixture;
use drawio_fixture::SIMPLE_DRAWIO_XML;
use sha2::{Digest, Sha256};
use std::thread;

#[test]
fn returns_not_installed_without_drawio_js() -> TestResult<()> {
    let missing_path = missing_runtime_path("drawio");

    let result = DrawioRendererOps::render_drawio_with_runtime_path(
        &drawio_block(SIMPLE_DRAWIO_XML),
        &missing_path,
        DiagramColorPreset::current(),
    );

    match result {
        DiagramResult::NotInstalled {
            kind, install_path, ..
        } => {
            assert_eq!(kind, "Draw.io");
            assert_eq!(install_path, missing_path);
        }
        other => {
            return Err(test_error(format!(
                "Expected Draw.io NotInstalled, got {other:?}"
            )));
        }
    }
    Ok(())
}

#[test]
fn resolve_drawio_js_prefers_env_var() -> TestResult<()> {
    let _guard = env_guard()?;
    let custom_path = PathBuf::from("/custom/drawio.min.js");
    let _env = EnvOverride::set("DRAWIO_JS", &custom_path);

    let path = DrawioRendererOps::resolve_drawio_js().map_err(std::io::Error::other)?;

    assert_eq!(path, custom_path);
    Ok(())
}

#[test]
fn invalid_drawio_javascript_does_not_fallback_to_native_svg() -> TestResult<()> {
    let invalid_runtime = InvalidRuntimeFile::create()?;

    let result = DrawioRendererOps::render_drawio_with_runtime_path(
        &drawio_block(SIMPLE_DRAWIO_XML),
        invalid_runtime.path(),
        DiagramColorPreset::current(),
    );

    assert!(matches!(result, DiagramResult::Err { .. }));
    Ok(())
}

#[test]
fn packaged_drawio_runtime_renders_svg() -> TestResult<()> {
    let _guard = env_guard()?;
    let drawio_js = DrawioRendererOps::find_drawio_js()
        .map_err(std::io::Error::other)?
        .ok_or_else(|| test_error("Packaged Draw.io runtime is unavailable"))?;
    let runtime_bytes = std::fs::read(&drawio_js)?;
    let runtime_checksum = Sha256::digest(&runtime_bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(runtime_checksum, DRAWIO_JS_CHECKSUM);

    let result = DrawioRendererOps::render_drawio_with_runtime_path(
        &drawio_block(SIMPLE_DRAWIO_XML),
        &drawio_js,
        DiagramColorPreset::current(),
    );

    let DiagramResult::Ok(svg) = result else {
        return Err(test_error(format!("Expected Draw.io SVG, got {result:?}")));
    };
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Box A"));
    Ok(())
}

#[test]
fn concurrent_drawio_renders_without_drawio_js() -> TestResult<()> {
    let missing_path = missing_runtime_path("drawio-concurrent");

    let handles = (0..3)
        .map(|index| {
            let runtime_path = missing_path.clone();
            thread::spawn(move || {
                let source = SIMPLE_DRAWIO_XML.replace("test", &format!("test-{index}"));
                let result = DrawioRendererOps::render_drawio_with_runtime_path(
                    &drawio_block(&source),
                    &runtime_path,
                    DiagramColorPreset::current(),
                );
                assert!(matches!(result, DiagramResult::NotInstalled { .. }));
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        match handle.join() {
            Ok(()) => {}
            Err(_) => return Err(test_error("Draw.io render thread panicked")),
        }
    }
    Ok(())
}

fn drawio_block(source: &str) -> DiagramBlock {
    DiagramBlock {
        kind: DiagramKind::DrawIo,
        source: source.to_string(),
    }
}

const INVALID_DRAWIO_JAVASCRIPT: &str = "function (";

struct InvalidRuntimeFile {
    path: PathBuf,
}

impl InvalidRuntimeFile {
    fn create() -> Result<Self, std::io::Error> {
        let path = std::env::temp_dir().join(format!(
            "kdr-drawio-invalid-runtime-{}.js",
            std::process::id()
        ));
        std::fs::write(&path, INVALID_DRAWIO_JAVASCRIPT)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for InvalidRuntimeFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
