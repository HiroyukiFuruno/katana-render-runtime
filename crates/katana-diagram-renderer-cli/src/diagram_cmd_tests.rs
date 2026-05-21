use super::{DiagramCommand, DiagramSourceOps, MermaidMarkdownOps, RenderInputFactory};
use crate::commands::{DiagramAction, ThemeModeArg};
use katana_diagram_renderer::DiagramKind;

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
        Some("/tmp/kdr-cache".into()),
    )?;

    assert_eq!(config["plantuml_theme"], "cyborg");
    assert_eq!(config["plantuml_theme_from"], "/path/to/themes");
    assert_eq!(config["plantuml_theme_mode"], "light");
    assert_eq!(config["plantuml_cache_dir"], "/tmp/kdr-cache");
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
fn mermaid_render_reports_missing_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::temp_dir().join(format!("kdr-cli-mmd-{}.md", std::process::id()));
    let output = std::env::temp_dir().join(format!("kdr-cli-mmd-{}.svg", std::process::id()));
    let runtime = std::env::temp_dir().join("missing-kdr-mermaid-runtime.js");

    std::fs::write(&input, "```mermaid\ngraph TD; A-->B\n```\n")?;
    let result = DiagramCommand::new(DiagramKind::Mermaid).run(DiagramAction::Render {
        input: input.clone(),
        output: Some(output),
        runtime: Some(runtime),
        theme: None,
        theme_from: None,
        theme_mode: None,
        cache_dir: None,
    });

    assert!(result.is_err());
    std::fs::remove_file(input)?;
    Ok(())
}

#[test]
fn drawio_render_reports_missing_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::temp_dir().join(format!("kdr-cli-drawio-{}.drawio", std::process::id()));
    let output = std::env::temp_dir().join(format!("kdr-cli-drawio-{}.svg", std::process::id()));
    let runtime = std::env::temp_dir().join("missing-kdr-drawio-runtime.js");

    std::fs::write(&input, "<mxGraphModel />")?;
    let result = DiagramCommand::new(DiagramKind::Drawio).run(DiagramAction::Render {
        input: input.clone(),
        output: Some(output),
        runtime: Some(runtime),
        theme: None,
        theme_from: None,
        theme_mode: None,
        cache_dir: None,
    });

    assert!(result.is_err());
    std::fs::remove_file(input)?;
    Ok(())
}
