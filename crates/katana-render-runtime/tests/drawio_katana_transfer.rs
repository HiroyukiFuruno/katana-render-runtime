use katana_render_runtime::markdown::color_preset::DiagramColorPreset;
use katana_render_runtime::markdown::drawio_renderer::DrawioRendererOps;
use katana_render_runtime::markdown::{DiagramBlock, DiagramKind, DiagramResult};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::thread;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

static ENV_LOCK: Mutex<()> = Mutex::new(());
const SIMPLE_DRAWIO_XML: &str = r#"<mxfile><diagram name="test"><mxGraphModel><root>
<mxCell id="0"/>
<mxCell id="1" parent="0"/>
<mxCell id="2" value="Box A" style="rounded=1;fillColor=#fff2cc;strokeColor=#d6b656;" vertex="1" parent="1">
    <mxGeometry x="80" y="80" width="120" height="60" as="geometry"/>
</mxCell>
<mxCell id="3" value="Box B" vertex="1" parent="1">
    <mxGeometry x="280" y="80" width="120" height="60" as="geometry"/>
</mxCell>
</root></mxGraphModel></diagram></mxfile>"#;

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
fn fake_drawio_js_does_not_fallback_to_native_svg() -> TestResult<()> {
    let fake_runtime = FakeRuntimeFile::create_invalid()?;

    let result = DrawioRendererOps::render_drawio_with_runtime_path(
        &drawio_block(SIMPLE_DRAWIO_XML),
        fake_runtime.path(),
        DiagramColorPreset::current(),
    );

    assert!(matches!(result, DiagramResult::Err { .. }));
    Ok(())
}

#[test]
fn fake_drawio_js_can_render_svg_without_official_runtime() -> TestResult<()> {
    let fake_runtime = FakeRuntimeFile::create_success()?;

    let result = DrawioRendererOps::render_drawio_with_runtime_path(
        &drawio_block(SIMPLE_DRAWIO_XML),
        fake_runtime.path(),
        DiagramColorPreset::current(),
    );

    let DiagramResult::Ok(svg) = result else {
        return Err(test_error("Expected fake Draw.io runtime to return SVG"));
    };
    assert!(svg.contains("drawio"));
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

fn missing_runtime_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kdr-{name}-missing-{}.js", std::process::id()))
}

fn env_guard() -> TestResult<MutexGuard<'static, ()>> {
    ENV_LOCK
        .lock()
        .map_err(|error| test_error(format!("environment lock is poisoned: {error}")))
}

struct EnvOverride {
    key: &'static str,
}

impl EnvOverride {
    fn set(key: &'static str, value: &Path) -> Self {
        unsafe { std::env::set_var(key, value) };
        Self { key }
    }
}

impl Drop for EnvOverride {
    fn drop(&mut self) {
        unsafe { std::env::remove_var(self.key) };
    }
}

struct FakeRuntimeFile {
    path: PathBuf,
}

impl FakeRuntimeFile {
    fn create_invalid() -> Result<Self, std::io::Error> {
        Self::create_with("invalid", "window.GraphViewer = {};")
    }

    fn create_success() -> Result<Self, std::io::Error> {
        Self::create_with("success", fake_success_runtime())
    }

    fn create_with(label: &str, source: &str) -> Result<Self, std::io::Error> {
        let path =
            std::env::temp_dir().join(format!("kdr-drawio-fake-{label}-{}.js", std::process::id()));
        std::fs::write(&path, source)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for FakeRuntimeFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn test_error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message.into()))
}

fn fake_success_runtime() -> &'static str {
    r#"
function Graph() {}
const Editor = { convertHtmlToText(value) { return String(value); } };
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  [["width", "20"], ["height", "10"], ["viewBox", "0 0 20 10"]]
    .forEach(([name, value]) => svg.setAttribute(name, value));
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.textContent = "drawio";
  svg.appendChild(text);
  callback({ graph: { getSvg() { return svg; } } });
};
"#
}
