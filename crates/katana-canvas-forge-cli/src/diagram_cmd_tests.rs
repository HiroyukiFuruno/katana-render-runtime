use super::{DiagramCommand, DiagramSourceOps, MermaidMarkdownOps, RenderInputFactory};
use crate::commands::DiagramAction;
use katana_canvas_forge::DiagramKind;

#[test]
fn extracts_mermaid_fence_from_markdown() {
    let source = "# Title\n\n~~~mermaid\ngraph TD; A-->B\n~~~\n".to_string();
    assert_eq!(MermaidMarkdownOps::extract(source), "graph TD; A-->B");
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
fn drawio_source_passes_through() {
    let source = "<mxGraphModel />".to_string();
    assert_eq!(
        DiagramSourceOps::prepare(DiagramKind::Drawio, source.clone()),
        source
    );
}

#[test]
fn render_input_factory_sets_kind_and_source() {
    let input = RenderInputFactory::create(DiagramKind::Mermaid, "graph TD; A".to_string());
    assert_eq!(input.kind, DiagramKind::Mermaid);
    assert_eq!(input.source, "graph TD; A");
}

#[test]
fn mermaid_render_reports_missing_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::temp_dir().join(format!("kcf-cli-mmd-{}.md", std::process::id()));
    let output = std::env::temp_dir().join(format!("kcf-cli-mmd-{}.svg", std::process::id()));
    let runtime = std::env::temp_dir().join("missing-kcf-mermaid-runtime.js");

    std::fs::write(&input, "```mermaid\ngraph TD; A-->B\n```\n")?;
    let result = DiagramCommand::new(DiagramKind::Mermaid).run(DiagramAction::Render {
        input: input.clone(),
        output,
        runtime: Some(runtime),
    });

    assert!(result.is_err());
    std::fs::remove_file(input)?;
    Ok(())
}

#[test]
fn drawio_render_reports_missing_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::temp_dir().join(format!("kcf-cli-drawio-{}.drawio", std::process::id()));
    let output = std::env::temp_dir().join(format!("kcf-cli-drawio-{}.svg", std::process::id()));
    let runtime = std::env::temp_dir().join("missing-kcf-drawio-runtime.js");

    std::fs::write(&input, "<mxGraphModel />")?;
    let result = DiagramCommand::new(DiagramKind::Drawio).run(DiagramAction::Render {
        input: input.clone(),
        output,
        runtime: Some(runtime),
    });

    assert!(result.is_err());
    std::fs::remove_file(input)?;
    Ok(())
}
