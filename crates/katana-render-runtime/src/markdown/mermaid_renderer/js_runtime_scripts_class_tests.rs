use super::MermaidRuntimeScripts;
use crate::markdown::{diagram_js_runtime::DiagramV8Runtime, runtime_assets::RuntimeAsset};

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

const CLASS_DIAGRAM_ENUMERATION_FIXTURE: &str = r#"classDiagram
    class PreviewPane {
        +full_render(source)
        +show_content(ui)
    }
    class RenderedSection {
        <<enumeration>>
        Markdown
        Image
        Error
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

#[test]
fn runtime_applies_enumeration_fixture_coordinate_patch_to_v11_and_v12_raw_viewboxes() {
    for (raw_view_box, expected_path) in [
        ("0 0 231.55450000000002 377", "M117.59,146L117.59,150.167"),
        (
            "4 4 231.55450000000002 367",
            "M121.58984375,150L121.58984375,184",
        ),
    ] {
        assert_enumeration_patch_applies(raw_view_box, expected_path);
    }
}

fn assert_enumeration_patch_applies(raw_view_box: &str, expected_path: &str) {
    let mermaid = fake_mermaid_with_enumeration_fixture_layout(raw_view_box);
    let rendered = render_class_fixture(&mermaid);
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|it| it.contains(expected_path) && !it.contains("M14-source")),
        "raw_view_box={raw_view_box}, rendered={rendered:?}"
    );
}

fn render_class_fixture(mermaid: &str) -> Result<String, String> {
    let scripts = MermaidRuntimeScripts::build_with_zenuml(mermaid, "", &class_request());
    DiagramV8Runtime::render(&scripts)
}

fn class_request() -> String {
    serde_json::json!({
        "source": CLASS_DIAGRAM_10_3,
        "svgId": "id",
        "theme": "dark",
        "background": "#000",
        "fill": "#111",
        "text": "#fff",
        "stroke": "#fff",
        "arrow": "#fff",
    })
    .to_string()
}

#[test]
fn mermaid12_enumeration_fixture_raw_viewbox_is_covered_by_the_coordinate_patch() {
    let request = serde_json::json!({
        "source": CLASS_DIAGRAM_ENUMERATION_FIXTURE,
        "svgId": "id",
        "theme": "dark",
        "background": "#000",
        "fill": "#111",
        "text": "#fff",
        "stroke": "#fff",
        "arrow": "#fff",
    })
    .to_string();
    let mermaid = mermaid12_bundle_that_returns_raw_viewbox();
    let scripts = MermaidRuntimeScripts::build_with_zenuml(&mermaid, "", &request);

    let rendered = DiagramV8Runtime::render(&scripts);

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|it| it == "4 4 231.55450000000002 367"),
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

fn mermaid12_bundle_that_returns_raw_viewbox() -> String {
    let bytes = match RuntimeAsset::mermaid().bytes() {
        Ok(bytes) => bytes,
        Err(_) => return String::new(),
    };
    let mermaid = match std::str::from_utf8(bytes) {
        Ok(mermaid) => mermaid,
        Err(_) => return String::new(),
    };
    format!(
        r#"{mermaid}
const katanaOriginalMermaidRender = globalThis.mermaid.render.bind(globalThis.mermaid);
globalThis.mermaid.render = async (...args) => {{
  const result = await katanaOriginalMermaidRender(...args);
  return {{ svg: result.svg.match(/<svg\b[^>]*\bviewBox="([^"]+)"/)?.[1] ?? "missing" }};
}};
"#
    )
}

fn fake_mermaid_with_enumeration_fixture_layout(raw_view_box: &str) -> String {
    format!(
        r##"
globalThis.mermaid = {{
  initialize() {{}},
  render: async (id) => {{
    const paths = Array.from({{ length: 15 }}, (_, index) => `<path d="M${{index}}-source"></path>`).join("");
    return {{ svg: `<svg id="${{id}}" width="100%" class="classDiagram" style="max-width: 231.55450000000002px;" viewBox="{raw_view_box}" role="graphics-document document" aria-roledescription="class"><g class="root" transform="translate(9, 9)"><g transform="translate(10, 10)"><text>PreviewPane RenderedSection «enumeration»</text></g>${{paths}}</g></svg>` }};
  }}
}};
"##
    )
}
