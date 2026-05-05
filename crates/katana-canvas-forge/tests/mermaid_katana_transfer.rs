use katana_canvas_forge::markdown::mermaid_renderer;
use katana_canvas_forge::markdown::svg_rasterize::SvgRasterizeOps;
use katana_canvas_forge::markdown::{DiagramBlock, DiagramKind, DiagramResult};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::thread;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
static ENV_LOCK: Mutex<()> = Mutex::new(());
const RUST_MANAGED_GANTT_MAX_WIDTH: u32 = 1800;
const JAPANESE_FLOWCHART_SOURCE: &str =
    "flowchart TD\n    A[\u{958b}\u{59cb}] --> B{\u{78ba}\u{8a8d}}\n    B --> C[\u{5b8c}\u{4e86}]";
const JAPANESE_KANBAN_SOURCE: &str = concat!(
    "---\n",
    "config:\n",
    "  kanban:\n",
    "    ticketBaseUrl: 'https://github.com/mermaid-js/mermaid/issues/#TICKET#'\n",
    "---\n",
    "kanban\n",
    "  \u{672a}\u{7740}\u{624b}\n",
    "    [\u{30c9}\u{30ad}\u{30e5}\u{30e1}\u{30f3}\u{30c8}\u{4f5c}\u{6210}]\n",
    "  [\u{9032}\u{884c}\u{4e2d}]\n",
    "    id6[\u{3059}\u{3079}\u{3066}\u{306e}\u{5834}\u{5408}\u{306b}\u{52d5}\u{4f5c}\u{3059}\u{308b}\u{30ec}\u{30f3}\u{30c0}\u{30e9}\u{30fc}\u{3092}\u{4f5c}\u{6210}\u{3059}\u{308b}\u{3002}\u{8868}\u{793a}\u{78ba}\u{8a8d}\u{306e}\u{305f}\u{3081}\u{3001}\u{9577}\u{3081}\u{306e}\u{30c6}\u{30ad}\u{30b9}\u{30c8}\u{3082}\u{5165}\u{308c}\u{3066}\u{3044}\u{308b}\u{3002}]\n",
    "  id11[\u{5b8c}\u{4e86}]\n",
    "    id5[\u{30c7}\u{30fc}\u{30bf}\u{53d6}\u{5f97}\u{3092}\u{5b9a}\u{7fa9}]\n",
);

#[test]
fn returns_not_installed_when_mermaid_js_is_missing() -> TestResult<()> {
    let missing_path = missing_runtime_path("mermaid");

    let result = mermaid_renderer::MermaidRenderOps::render_mermaid_with_runtime_path(
        &mermaid_block(),
        &missing_path,
    );

    match result {
        DiagramResult::NotInstalled {
            kind,
            download_url,
            install_path,
        } => {
            assert_eq!(kind, "Mermaid");
            assert!(download_url.contains("mermaid.min.js"));
            assert_eq!(install_path, missing_path);
        }
        other => {
            return Err(test_error(format!(
                "Expected NotInstalled when Mermaid.js is missing, got {other:?}"
            )));
        }
    }
    Ok(())
}

#[test]
fn resolve_mermaid_js_prefers_env_var() -> TestResult<()> {
    let _guard = env_guard()?;
    let custom_path = PathBuf::from("/custom/mermaid.min.js");
    let _env = EnvOverride::set("MERMAID_JS", &custom_path);
    let path =
        mermaid_renderer::MermaidBinaryOps::resolve_mermaid_js().map_err(std::io::Error::other)?;

    assert_eq!(path, custom_path);
    Ok(())
}

#[test]
fn find_mermaid_js_returns_none_for_missing_env_path() -> TestResult<()> {
    let _guard = env_guard()?;
    let missing_path = missing_runtime_path("mermaid-env");
    let _env = EnvOverride::set("MERMAID_JS", &missing_path);
    let path = mermaid_renderer::MermaidBinaryOps::find_mermaid_js()?;

    assert!(path.is_none());
    Ok(())
}

#[test]
fn gantt_future_today_marker_does_not_expand_canvas_when_runtime_is_available() -> TestResult<()> {
    let _guard = env_guard()?;
    if mermaid_renderer::MermaidBinaryOps::find_mermaid_js()?.is_none() {
        return Ok(());
    }

    let with_marker = rasterized_dimensions(&render_svg(gantt_source(""))?)?;
    let without_marker = rasterized_dimensions(&render_svg(gantt_source("todayMarker off"))?)?;

    assert_eq!(with_marker, without_marker);
    assert!(with_marker.0 <= RUST_MANAGED_GANTT_MAX_WIDTH);
    Ok(())
}

#[test]
fn japanese_labels_render_when_runtime_is_available() -> TestResult<()> {
    let _guard = env_guard()?;
    if mermaid_renderer::MermaidBinaryOps::find_mermaid_js()?.is_none() {
        return Ok(());
    }

    let flowchart_svg = render_svg(JAPANESE_FLOWCHART_SOURCE.to_string())?;
    let kanban_svg = render_svg(JAPANESE_KANBAN_SOURCE.to_string())?;

    assert!(flowchart_svg.contains("\u{958b}\u{59cb}"));
    assert!(flowchart_svg.contains("\u{78ba}\u{8a8d}"));
    assert!(kanban_svg.contains("\u{672a}\u{7740}\u{624b}"));
    assert!(kanban_svg.contains("\u{9032}\u{884c}\u{4e2d}"));
    Ok(())
}

#[test]
fn concurrent_mermaid_rendering_succeeds_when_runtime_is_available() -> TestResult<()> {
    let _guard = env_guard()?;
    if mermaid_renderer::MermaidBinaryOps::find_mermaid_js()?.is_none() {
        return Ok(());
    }

    let handles = mermaid_sources()
        .into_iter()
        .map(|source| thread::spawn(move || render_svg(source.to_string())))
        .collect::<Vec<_>>();

    for handle in handles {
        let svg = match handle.join() {
            Ok(result) => result?,
            Err(_) => return Err(test_error("Mermaid render thread panicked")),
        };
        assert!(svg.contains("<svg"));
        let dimensions = rasterized_dimensions(&svg)?;
        assert!(dimensions.0 > 0);
        assert!(dimensions.1 > 0);
    }
    Ok(())
}

fn mermaid_block() -> DiagramBlock {
    DiagramBlock {
        kind: DiagramKind::Mermaid,
        source: "graph TD; A-->B".to_string(),
    }
}

fn mermaid_sources() -> [&'static str; 4] {
    [
        "graph TD; A-->B",
        "sequenceDiagram\n  participant User\n  participant KatanA\n  User->>KatanA: Open",
        "classDiagram\n  class PreviewPane\n  PreviewPane --> RenderedSection",
        "stateDiagram-v2\n  [*] --> Pending\n  Pending --> Image : success",
    ]
}

fn gantt_source(today_marker: &str) -> String {
    format!(
        "gantt\n\
         title Katana schedule\n\
         dateFormat YYYY-MM-DD\n\
         {today_marker}\n\
         section Core\n\
         Markdown rendering :done, a1, 2026-01-04, 2026-01-17\n\
         Diagram support :a2, 2026-02-01, 2026-02-15\n\
         Preview pane :a3, 2026-01-18, 2026-02-15"
    )
}

fn render_svg(source: String) -> TestResult<String> {
    let Some(mermaid_js) = mermaid_renderer::MermaidBinaryOps::find_mermaid_js()? else {
        return Err(test_error("Mermaid.js must exist before rendering"));
    };
    let block = DiagramBlock {
        kind: DiagramKind::Mermaid,
        source,
    };
    match mermaid_renderer::MermaidRenderOps::render_mermaid_with_runtime_path(&block, &mermaid_js)
    {
        DiagramResult::Ok(svg) => Ok(svg),
        other => Err(test_error(format!(
            "Expected Mermaid SVG rendering, got {other:?}"
        ))),
    }
}

fn rasterized_dimensions(svg: &str) -> TestResult<(u32, u32)> {
    let image = SvgRasterizeOps::rasterize_svg(svg, 1.0)?;
    Ok((image.width, image.height))
}

fn missing_runtime_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kcf-{name}-missing-{}.js", std::process::id()))
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
    fn set(key: &'static str, value: &std::path::Path) -> Self {
        unsafe { std::env::set_var(key, value) };
        Self { key }
    }
}

impl Drop for EnvOverride {
    fn drop(&mut self) {
        unsafe { std::env::remove_var(self.key) };
    }
}

fn test_error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message.into()))
}
