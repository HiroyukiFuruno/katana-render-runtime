use crate::markdown::diagram_js_runtime::DiagramRuntimeScript;

pub(super) struct MermaidRuntimeScripts;

impl MermaidRuntimeScripts {
    pub(super) fn build<'a>(bundle: &'a str, request_json: &str) -> Vec<DiagramRuntimeScript<'a>> {
        Self::build_with_zenuml(bundle, MERMAID_ZENUML, request_json)
    }

    fn build_with_zenuml<'a>(
        bundle: &'a str,
        zenuml_bundle: &'a str,
        request_json: &str,
    ) -> Vec<DiagramRuntimeScript<'a>> {
        vec![
            DiagramRuntimeScript::borrowed("mermaid-runtime.min.js", MERMAID_RUNTIME),
            DiagramRuntimeScript::borrowed("mermaid.min.js", bundle),
            DiagramRuntimeScript::borrowed("mermaid-zenuml.min.js", zenuml_bundle),
            DiagramRuntimeScript::owned("render-mermaid.js", render_script(request_json)),
        ]
    }
}

fn render_script(request_json: &str) -> String {
    format!("katanaInstallMermaidZenumlRuntimeAdapter();\nkatanaRunMermaidRuntime({request_json});")
}

const MERMAID_RUNTIME: &str = include_str!("../diagram_runtime/generated/mermaid-runtime.min.js");
const MERMAID_ZENUML: &str =
    include_str!("../../../vendor/mermaid-zenuml/0.2.2/mermaid-zenuml.min.js");

#[cfg(test)]
mod tests {
    use super::MermaidRuntimeScripts;
    use crate::markdown::diagram_js_runtime::DiagramV8Runtime;

    #[test]
    fn build_includes_bundle_and_render_script() {
        let scripts = MermaidRuntimeScripts::build("bundle", "{}");
        assert!(scripts.iter().any(|it| it.name == "mermaid-runtime.min.js"));
        assert!(scripts.iter().any(|it| it.name == "mermaid.min.js"));
        assert!(scripts.iter().any(|it| it.name == "mermaid-zenuml.min.js"));
        assert!(scripts.iter().any(|it| it.name == "render-mermaid.js"));
    }

    #[test]
    fn zenuml_registration_runs_before_render() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid(),
            fake_zenuml(),
            r##"{"source":"zenuml\nA.method()","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"zenuml"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(|it| it.contains("registered")),
            "{rendered:?}"
        );
    }

    #[test]
    fn zenuml_directive_source_registers_external_diagram_without_request_hint() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid(),
            fake_zenuml(),
            r##"{"source":"%%{init: { \"theme\": \"dark\" }}%%\n%% comment\nzenuml\nA.method()","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(|it| it.contains("registered")),
            "{rendered:?}"
        );
    }

    fn fake_mermaid() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  registerExternalDiagrams: async (diagrams) => {
    globalThis.__registeredDiagram = diagrams[0].id;
  },
  render: async (id) => {
    const text = globalThis.__registeredDiagram ?? "missing";
    return { svg: `<svg id="${id}"><text>${text}</text></svg>` };
  }
};
"#
    }

    fn fake_zenuml() -> &'static str {
        r#"globalThis["mermaid-zenuml"] = { id: "registered" };"#
    }
}
