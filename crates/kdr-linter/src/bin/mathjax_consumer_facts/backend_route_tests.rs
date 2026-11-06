use super::*;

const BACKEND_ROUTE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "if block.source.trim().is_empty() {",
        "if block.source.is_empty() {",
        "backend-runtime-path:empty-source",
    ),
    (
        "return (Err(\"MathJax source is empty\".to_string()), None);",
        "return (Err(\"other\".to_string()), None);",
        "backend-runtime-path:empty-source",
    ),
    (
        "return (Err(\"MathJax source is empty\".to_string()), None);",
        "return (Err(\"MathJax source is empty\".to_string()), Some(other));",
        "backend-runtime-path:empty-source",
    ),
    (
        "Ok(resource) => resource,",
        "Ok(resource) => other,",
        "backend-runtime-path:resource-errors",
    ),
    (
        "Err(error) => return (Err(error), None),",
        "Err(error) => return (Err(error), Some(other)),",
        "backend-runtime-path:resource-errors",
    ),
    (
        "Err(error) => return (Err(error), None),",
        "Err(error) => return (Err(error), None),\n            _ => other,",
        "backend-runtime-path:resource-load",
    ),
    (
        "Err(error) => return (Err(error), None),",
        "Err(error) => other,",
        "backend-runtime-path:resource-errors",
    ),
    (
        "Err(error) => return (Err(error), None),",
        "Err(error) => return Err(error),",
        "backend-runtime-path:resource-errors",
    ),
    (
        ".map_err(|error| self.resource_error(error))",
        ".map_err(|error| self.resource_error(other))",
        "backend-runtime-path:resource-load",
    ),
    (
        ".map_err(|error| self.resource_error(error))",
        ".map_err(|other| self.resource_error(error))",
        "backend-runtime-path:resource-load",
    ),
    (
        "if error.kind() == std::io::ErrorKind::NotFound {",
        "if error.kind() != std::io::ErrorKind::NotFound {",
        "backend-runtime-path:resource-errors",
    ),
    (
        "MathJax runtime asset is not installed: {}",
        "MathJax runtime asset is unavailable: {}",
        "backend-runtime-path:resource-errors",
    ),
    (
        "MathJax runtime asset could not be read: {}: {error}",
        "MathJax runtime asset could not be loaded: {}: {error}",
        "backend-runtime-path:resource-errors",
    ),
    (
        "let (result, digest) = self.render_block(input);",
        "let (result, other) = self.render_block(input);",
        "backend-runtime-path:render-dispatch",
    ),
    (
        ".map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        ".map(|output| RenderOutputFactory::bind_resource(output, other.as_deref()))",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        ".map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        ".map(|other| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "format!(\"error:{}:{warning}\", digest.as_deref().unwrap_or(\"\"))",
        "format!(\"error:{}:{warning}\", other.as_deref().unwrap_or(\"\"))",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "Some(&error_key),",
        "Some(&other),",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "runtime_resource_loader::load(",
        "runtime_resource_loader::read(",
        "backend-runtime-path:resource-load",
    ),
    (
        "self.bundled.then_some(BundledResource::MathJax)",
        "self.bundled.then_some(BundledResource::Drawio)",
        "backend-runtime-path:resource-load",
    ),
    (
        "resource.into_digest()",
        "other.into_digest()",
        "backend-runtime-path:resource-digest",
    ),
    (
        "            &resource,\n        );",
        "            &other,\n        );",
        "backend-runtime-path:arguments",
    ),
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
        "MathJaxRendererOps::render_mathjax_with_loaded(",
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
