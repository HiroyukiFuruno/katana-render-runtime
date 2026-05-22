use katana_render_runtime::{
    MathJaxRenderer, RenderConfig, RenderContext, RenderInput, RenderKind, RenderPolicy,
    RenderThemeMode, RenderThemeSnapshot, Renderer, RuntimePathResolver,
};
use serde_json::json;

#[test]
fn mathjax_inline_tex_renders_svg() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = renderer()?;
    let output = renderer.render(&input("E = mc^2", false))?;

    assert!(output.svg.contains("<svg"), "{:?}", output.diagnostics);
    assert!(output.svg.contains("viewBox"));
    assert!(output.width > 0.0);
    assert!(output.height > 0.0);
    assert!(output.diagnostics.errors.is_empty());
    assert!(output.svg.contains("<path"));
    Ok(())
}

#[test]
fn mathjax_display_tex_renders_svg_with_dimensions() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = renderer()?;
    let output = renderer.render(&input(r"\int_{0}^{x} \frac{t^2}{1 + t^4} \, dt", true))?;

    assert!(output.svg.contains("<svg"), "{:?}", output.diagnostics);
    assert!(output.view_box.contains(' '));
    assert!(output.width > 0.0);
    assert!(output.height > 0.0);
    assert!(output.svg.contains("<path"));
    Ok(())
}

#[test]
fn mathjax_reports_invalid_tex_as_raw_string() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = renderer()?;
    let output = renderer.render(&input(r"\not_a_real_mathjax_command", false))?;

    assert_eq!(output.svg, r"\not_a_real_mathjax_command");
    assert_eq!(output.width, 0.0);
    assert_eq!(output.height, 0.0);
    assert!(!output.diagnostics.errors.is_empty());
    Ok(())
}

#[test]
fn mathjax_missing_runtime_path_returns_raw_string() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = MathJaxRenderer::with_runtime_path(
        std::env::temp_dir().join("missing-katana-render-runtime-mathjax.js"),
    );
    let output = renderer.render(&input("E = mc^2", false))?;

    assert_eq!(output.svg, "E = mc^2");
    assert_eq!(output.width, 0.0);
    assert_eq!(output.height, 0.0);
    assert!(output.diagnostics.errors[0].contains("MathJax runtime asset is not installed"));
    Ok(())
}

#[test]
fn mathjax_uses_explicit_runtime_path() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = TempMathJaxRuntime::create()?;
    let renderer = MathJaxRenderer::with_runtime_path(runtime.path.clone());
    let output = renderer.render(&input("custom runtime", false))?;

    assert!(output.svg.contains("data-runtime=\"custom\""));
    assert!(output.diagnostics.errors.is_empty());
    Ok(())
}

#[test]
fn mathjax_theme_changes_cache_fingerprint() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = renderer()?;
    let light = renderer.render(&input_with_theme(RenderThemeMode::Light))?;
    let dark = renderer.render(&input_with_theme(RenderThemeMode::Dark))?;

    assert_ne!(light.cache_fingerprint, dark.cache_fingerprint);
    Ok(())
}

fn renderer() -> Result<MathJaxRenderer, Box<dyn std::error::Error>> {
    Ok(MathJaxRenderer::with_runtime_path(
        RuntimePathResolver::resolve(RenderKind::MathJax, None)?,
    ))
}

fn input(source: &str, display: bool) -> RenderInput {
    RenderInput {
        kind: RenderKind::MathJax,
        source: source.to_string(),
        config: RenderConfig {
            vendor_config: json!({ "display": display }),
        },
        policy: RenderPolicy::default(),
        context: RenderContext::default(),
    }
}

fn input_with_theme(mode: RenderThemeMode) -> RenderInput {
    RenderInput {
        kind: RenderKind::MathJax,
        source: r"\sum_{n=1}^{10} n^2".to_string(),
        config: RenderConfig {
            vendor_config: json!({ "display": true }),
        },
        policy: RenderPolicy::default(),
        context: RenderContext {
            theme_fingerprint: None,
            document_id: None,
            theme: Some(theme(mode)),
        },
    }
}

fn theme(mode: RenderThemeMode) -> RenderThemeSnapshot {
    let palette = ThemePalette::for_mode(mode);
    RenderThemeSnapshot {
        mode,
        background: palette.background.to_string(),
        text: palette.text.to_string(),
        fill: palette.fill.to_string(),
        stroke: palette.stroke.to_string(),
        arrow: palette.stroke.to_string(),
        drawio_label_color: palette.stroke.to_string(),
        mermaid_theme: palette.mermaid_theme.to_string(),
        plantuml_class_bg: palette.fill.to_string(),
        plantuml_note_bg: palette.note_bg.to_string(),
        plantuml_note_text: palette.text.to_string(),
        syntax_theme_dark: "base16-ocean.dark".to_string(),
        syntax_theme_light: "base16-ocean.light".to_string(),
        preview_text: palette.text.to_string(),
    }
}

struct ThemePalette {
    background: &'static str,
    text: &'static str,
    fill: &'static str,
    stroke: &'static str,
    mermaid_theme: &'static str,
    note_bg: &'static str,
}

struct TempMathJaxRuntime {
    path: std::path::PathBuf,
}

impl TempMathJaxRuntime {
    fn create() -> Result<Self, Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "katana-render-runtime-mathjax-custom-{}.js",
            std::process::id()
        ));
        std::fs::write(&path, CUSTOM_MATHJAX_RUNTIME)?;
        Ok(Self { path })
    }
}

impl Drop for TempMathJaxRuntime {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

const CUSTOM_MATHJAX_RUNTIME: &str = r#"
function katanaRunMathJaxRuntime(request) {
  return JSON.stringify({
    kind: "svg",
    svg: `<svg xmlns="http://www.w3.org/2000/svg" width="12" height="8" viewBox="0 0 12 8" data-runtime="custom"><text>${request.source}</text></svg>`
  });
}
"#;

impl ThemePalette {
    fn for_mode(mode: RenderThemeMode) -> Self {
        match mode {
            RenderThemeMode::Light => Self::light(),
            RenderThemeMode::Dark => Self::dark(),
        }
    }

    fn light() -> Self {
        Self {
            background: "#ffffff",
            text: "#111111",
            fill: "#f7fbff",
            stroke: "#222222",
            mermaid_theme: "default",
            note_bg: "#fff7cc",
        }
    }

    fn dark() -> Self {
        Self {
            background: "#111111",
            text: "#f5f5f5",
            fill: "#243447",
            stroke: "#f5f5f5",
            mermaid_theme: "dark",
            note_bg: "#3d3522",
        }
    }
}
