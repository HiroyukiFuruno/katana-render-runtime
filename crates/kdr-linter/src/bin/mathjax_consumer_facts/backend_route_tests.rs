use super::*;

const BACKEND_ROUTE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "        let preset = DiagramColorPreset::for_render_input(input);",
        "        let _extra = ();\n        let preset = DiagramColorPreset::for_render_input(input);",
        "backend-runtime-path:statement-count",
    ),
    (
        "        let block = DiagramBlock {\n            kind: crate::markdown::DiagramKind::MathJax,\n            source: input.source.clone(),\n        };",
        "        let block = input.source.clone();",
        "backend-runtime-path:block",
    ),
    (
        "            source: input.source.clone(),\n        };",
        "            source: input.source,\n        };",
        "backend-runtime-path:block-fields",
    ),
    (
        "            source: input.source.clone(),\n        };",
        "            source: input.source.clone(),\n            extra: input.source.clone(),\n        };",
        "backend-runtime-path:block",
    ),
    (
        "DiagramColorPreset::for_render_input(input)",
        "DiagramColorPreset::for_render_input(other_input)",
        "backend-runtime-path:preset-input",
    ),
    (
        "MathJaxRendererOps::render_mathjax_with_runtime_path(",
        "MathJaxRendererOps::render_mathjax_with_runtime(",
        "backend-runtime-path:arguments",
    ),
    (
        "MathJaxRenderConfig::display(input)",
        "MathJaxRenderConfig::display(other_input)",
        "backend-runtime-path:arguments",
    ),
    (
        "if input.kind != DiagramKind::MathJax {",
        "if input.kind == DiagramKind::MathJax {",
        "backend-runtime-path:render-kind-check",
    ),
    (
        "if input.kind != DiagramKind::MathJax {",
        "if let _kind = input.kind {",
        "backend-runtime-path:render-kind-check",
    ),
    (
        "RuntimeDescriptor::mathjax()",
        "RuntimeDescriptor::native()",
        "backend-runtime-path:render-runtime",
    ),
    (
        "let runtime = RuntimeDescriptor::mathjax();",
        "let runtime;",
        "backend-runtime-path:render-runtime",
    ),
    (
        "Ok(svg) => RenderOutputFactory::from_diagram_result(",
        "Err(svg) => RenderOutputFactory::from_diagram_result(",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "Ok(svg) => RenderOutputFactory::from_diagram_result(",
        "Ok(other) => RenderOutputFactory::from_diagram_result(",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "kind: crate::markdown::DiagramKind::MathJax,",
        "kind: input.kind,",
        "backend-runtime-path:block-fields",
    ),
];

#[test]
fn rejects_backend_route_changes_at_their_contract_boundary()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let relative = RUST_INPUTS[4];
    let source = std::fs::read_to_string(repository_root()?.join(relative))?;
    for (before, _, _) in BACKEND_ROUTE_MUTATIONS {
        assert_eq!(
            source.matches(before).count(),
            1,
            "mutation anchor: {before}"
        );
    }
    super::call_binding_tests::reject_mutations(&fixture, relative, BACKEND_ROUTE_MUTATIONS)
}
