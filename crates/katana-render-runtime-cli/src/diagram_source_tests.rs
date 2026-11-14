use super::{DiagramSourceOps, MermaidMarkdownOps};
use katana_render_runtime::DiagramKind;

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
fn closes_fence_only_with_matching_marker_and_sufficient_length() {
    let source = "````mermaid\ngraph TD\n```\n~~~\nA-->B\n`````  \n".to_string();
    assert_eq!(
        MermaidMarkdownOps::extract(source),
        "graph TD\n```\n~~~\nA-->B"
    );
}

#[test]
fn keeps_source_when_only_short_or_mismatched_closers_exist() {
    let source = "~~~~plantuml\n@startuml\n~~~\n````\n@enduml\n".to_string();
    assert_eq!(
        DiagramSourceOps::prepare(DiagramKind::PlantUml, source.clone()),
        source
    );
}

#[test]
fn extracts_long_tilde_fence_with_matching_closer() {
    let source = "~~~~plantuml\n@startuml\n@enduml\n~~~~\n".to_string();
    assert_eq!(
        DiagramSourceOps::prepare(DiagramKind::PlantUml, source),
        "@startuml\n@enduml"
    );
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
