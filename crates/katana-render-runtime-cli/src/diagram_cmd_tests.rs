use super::{DiagramCommand, DiagramSourceOps, MermaidMarkdownOps, RenderInputFactory};
use crate::commands::{DiagramAction, ThemeModeArg};
use katana_render_runtime::DiagramKind;
use std::path::PathBuf;
use tempfile::TempDir;

struct InputFixture {
    _directory: TempDir,
    root: PathBuf,
    input: PathBuf,
    output: PathBuf,
}

impl InputFixture {
    fn new(file_name: &str, source: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = TempDir::new()?;
        let root = directory.path().canonicalize()?;
        let input = root.join(file_name);
        let output = root.join("output.svg");
        std::fs::write(&input, source)?;
        Ok(Self {
            _directory: directory,
            root,
            input,
            output,
        })
    }
}

#[test]
fn extracts_mermaid_fence_from_markdown() {
    let source = "# Title\n\n~~~mermaid\ngraph TD; A-->B\n~~~\n".to_string();
    assert_eq!(MermaidMarkdownOps::extract(source), "graph TD; A-->B");
}

#[test]
fn extracts_zenuml_fence_and_prepends_zenuml_keyword() {
    let source = "~~~zenuml\ntitle Flow\nA.method()\n~~~\n".to_string();
    assert_eq!(
        MermaidMarkdownOps::extract(source),
        "zenuml\ntitle Flow\nA.method()"
    );
}

#[test]
fn extracts_zenuml_direct_fence_from_committed_fixture() {
    let fixture =
        include_str!("../../../tests/fixtures/mermaid/en/29-zenuml-direct.md").to_string();
    let extracted = MermaidMarkdownOps::extract(fixture);
    assert!(
        extracted.starts_with("zenuml\n"),
        "extracted source should start with 'zenuml\\n', got: {extracted:?}"
    );
}

#[test]
fn extracts_mermaid_fence_with_info_string_attributes() {
    let source = "``` mermaid title=\"flow\"\ngraph TD; A-->B\n```\n".to_string();
    assert_eq!(MermaidMarkdownOps::extract(source), "graph TD; A-->B");
}

#[test]
fn extracts_mermaid_fence_when_language_has_attributes() {
    let source = "```mermaid title=flow\ngraph TD; A-->B\n```\n".to_string();
    assert_eq!(MermaidMarkdownOps::extract(source), "graph TD; A-->B");
}

#[test]
fn keeps_source_when_mermaid_fence_is_not_closed() {
    let source = "```mermaid\ngraph TD; A-->B".to_string();
    assert_eq!(MermaidMarkdownOps::extract(source.clone()), source);
}

#[test]
fn unknown_fence_language_passes_through_as_source() {
    let source = "```javascript\nconsole.log('hi')\n```\n".to_string();
    assert_eq!(MermaidMarkdownOps::extract(source.clone()), source);
}

#[test]
fn drawio_source_passes_through() {
    let source = "<mxGraphModel />".to_string();
    assert_eq!(
        DiagramSourceOps::prepare(DiagramKind::Drawio, source.clone()),
        source
    );
}

#[test]
fn mathjax_source_passes_through() {
    let source = "x^2".to_string();
    assert_eq!(
        DiagramSourceOps::prepare(DiagramKind::MathJax, source.clone()),
        source
    );
}

#[test]
fn extracts_plantuml_fence_from_markdown() {
    let source = "```plantuml\n@startuml\nAlice -> Bob: hello\n@enduml\n```\n".to_string();
    assert_eq!(
        DiagramSourceOps::prepare(DiagramKind::PlantUml, source),
        "@startuml\nAlice -> Bob: hello\n@enduml"
    );
}

#[test]
fn render_input_factory_sets_kind_and_source() {
    let input = RenderInputFactory::create(
        DiagramKind::Mermaid,
        "graph TD; A".to_string(),
        serde_json::Value::Null,
    );
    assert_eq!(input.kind, DiagramKind::Mermaid);
    assert_eq!(input.source, "graph TD; A");
}

#[test]
fn plantuml_theme_options_become_vendor_config() -> Result<(), Box<dyn std::error::Error>> {
    let config = RenderInputFactory::vendor_config(
        DiagramKind::PlantUml,
        Some("cyborg".to_string()),
        Some("/path/to/themes".to_string()),
        Some(ThemeModeArg::Light),
        Some("/tmp/krr-cache".into()),
    )?;

    assert_eq!(config["plantuml_theme"], "cyborg");
    assert_eq!(config["plantuml_theme_from"], "/path/to/themes");
    assert_eq!(config["plantuml_theme_mode"], "light");
    assert_eq!(config["plantuml_cache_dir"], "/tmp/krr-cache");
    Ok(())
}

#[test]
fn plantuml_theme_options_without_cache_dir_use_empty_cache_config()
-> Result<(), Box<dyn std::error::Error>> {
    let config = RenderInputFactory::vendor_config(
        DiagramKind::PlantUml,
        Some("cyborg".to_string()),
        None,
        Some(ThemeModeArg::Dark),
        None,
    )?;

    assert_eq!(config["plantuml_theme"], "cyborg");
    assert_eq!(config["plantuml_theme_from"], "");
    assert_eq!(config["plantuml_theme_mode"], "dark");
    assert_eq!(config["plantuml_cache_dir"], "");
    Ok(())
}

#[test]
fn non_plantuml_rejects_theme_options() {
    let result = RenderInputFactory::vendor_config(
        DiagramKind::Mermaid,
        Some("dark".to_string()),
        None,
        None,
        None,
    );

    assert!(result.is_err());
}

#[test]
fn runtime_option_validation_and_renderer_selection_cover_all_kinds() {
    let conflict = DiagramCommand::validate_runtime_options(
        DiagramKind::PlantUml,
        Some(&"runtime.jar".into()),
        Some(&"cache".into()),
    );
    let accepted = DiagramCommand::validate_runtime_options(DiagramKind::Mermaid, None, None);
    let _mermaid = DiagramCommand::new(DiagramKind::Mermaid).renderer("mermaid.js".into());
    let _drawio = DiagramCommand::new(DiagramKind::Drawio).renderer("drawio.js".into());
    let _plantuml = DiagramCommand::new(DiagramKind::PlantUml).renderer("plantuml.jar".into());
    let _mathjax = DiagramCommand::new(DiagramKind::MathJax).renderer("mathjax.js".into());

    assert!(conflict.is_err());
    assert!(accepted.is_ok());
}

#[test]
fn plantuml_render_rejects_runtime_and_cache_conflict_before_reading_input() {
    let result = DiagramCommand::new(DiagramKind::PlantUml).run(DiagramAction::Render {
        source_options: crate::commands::DiagramSourceOptions::default(),
        input: "input-is-not-read.puml".into(),
        output: None,
        runtime: Some("runtime.jar".into()),
        theme: None,
        theme_from: None,
        theme_mode: None,
        cache_dir: Some("cache".into()),
    });

    assert!(
        matches!(result, Err(error) if error.to_string().contains("--runtime and --cache-dir"))
    );
}

#[test]
fn render_rejects_theme_options_for_non_plantuml_after_reading_input()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = InputFixture::new("input.mmd", "graph TD; A --> B")?;
    let result = DiagramCommand::new(DiagramKind::Mermaid).run(DiagramAction::Render {
        source_options: crate::commands::DiagramSourceOptions::default(),
        input: fixture.input.clone(),
        output: None,
        runtime: Some("runtime.js".into()),
        theme: Some("not-supported".to_string()),
        theme_from: None,
        theme_mode: None,
        cache_dir: None,
    });

    assert!(
        matches!(result, Err(error) if error.to_string().contains("currently supported only for plantuml"))
    );
    Ok(())
}

#[test]
fn vendor_config_is_null_without_plantuml_options() -> Result<(), Box<dyn std::error::Error>> {
    let config = RenderInputFactory::vendor_config(DiagramKind::Mermaid, None, None, None, None)?;

    assert!(config.is_null());
    Ok(())
}

#[test]
fn plantuml_render_reaches_config_validation_after_input_loading()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = InputFixture::new("input.puml", "@startuml\nAlice -> Bob\n@enduml")?;
    let result = DiagramCommand::new(DiagramKind::PlantUml).run(DiagramAction::Render {
        source_options: crate::commands::DiagramSourceOptions::default(),
        input: fixture.input.clone(),
        output: None,
        runtime: Some(fixture.root.join("missing-plantuml.jar")),
        theme: Some("bad theme".to_string()),
        theme_from: None,
        theme_mode: None,
        cache_dir: None,
    });

    assert!(result.is_err());
    Ok(())
}

fn assert_missing_runtime_render(
    kind: DiagramKind,
    fixture: InputFixture,
    runtime_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let result = DiagramCommand::new(kind).run(DiagramAction::Render {
        source_options: crate::commands::DiagramSourceOptions::default(),
        input: fixture.input.clone(),
        output: Some(fixture.output.clone()),
        runtime: Some(fixture.root.join(runtime_name)),
        theme: None,
        theme_from: None,
        theme_mode: None,
        cache_dir: None,
    });

    assert!(result.is_err());
    Ok(())
}

#[test]
fn mermaid_render_reports_missing_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = InputFixture::new("input.md", "```mermaid\ngraph TD; A-->B\n```\n")?;
    assert_missing_runtime_render(
        DiagramKind::Mermaid,
        fixture,
        "missing-krr-mermaid-runtime.js",
    )
}

#[test]
fn drawio_render_reports_missing_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = InputFixture::new("input.drawio", "<mxGraphModel />")?;
    assert_missing_runtime_render(
        DiagramKind::Drawio,
        fixture,
        "missing-krr-drawio-runtime.js",
    )
}
#[path = "diagram_cmd_attestation_tests.rs"]
mod attestation_tests;
