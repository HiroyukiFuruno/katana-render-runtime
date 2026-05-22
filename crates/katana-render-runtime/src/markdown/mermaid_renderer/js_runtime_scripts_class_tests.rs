use super::MermaidRuntimeScripts;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;

const CLASS_DIAGRAM_10_3: &str = r#"classDiagram
    class PreviewPane {
        +Vec~RenderedSection~ sections
        +full_render(source, path)
        +wait_for_renders()
        +show_content(ui)
    }
    class RenderedSection {
        <<enumeration>>
        Markdown
        Image
        Error
        CommandNotFound
        NotInstalled
        Pending
    }
    PreviewPane --> RenderedSection"#;

#[test]
fn runtime_keeps_large_class_diagram_without_fixture_coordinate_patch() {
    let request = serde_json::json!({
        "source": CLASS_DIAGRAM_10_3,
        "svgId": "id",
        "theme": "dark",
        "background": "#000",
        "fill": "#111",
        "text": "#fff",
        "stroke": "#fff",
        "arrow": "#fff",
    })
    .to_string();
    let scripts = MermaidRuntimeScripts::build_with_zenuml(
        fake_mermaid_with_large_class_enumeration_layout(),
        "",
        &request,
    );

    let rendered = DiagramV8Runtime::render(&scripts);

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|it| it.contains(r#"max-width: 420px;"#)
                && it.contains(r#"viewBox="-8 -8 420 620""#)
                && it.contains("M14-source")
                && !it.contains("M117.59,146L117.59,150.167")),
        "{rendered:?}"
    );
}

fn fake_mermaid_with_large_class_enumeration_layout() -> &'static str {
    r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const paths = Array.from({ length: 15 }, (_, index) => `<path d="M${index}-source"></path>`).join("");
    return { svg: `<svg id="${id}" width="100%" class="classDiagram" style="max-width: 420px;" viewBox="-8 -8 420 620" role="graphics-document document" aria-roledescription="class"><g class="root" transform="translate(9, 9)"><g transform="translate(10, 10)"><text>PreviewPane RenderedSection «enumeration» CommandNotFound NotInstalled Pending</text></g>${paths}</g></svg>` };
  }
};
"##
}
