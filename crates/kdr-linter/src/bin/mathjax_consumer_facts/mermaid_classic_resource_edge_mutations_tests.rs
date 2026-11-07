pub(super) const BACKEND_CASES: &[(&str, &str, &str)] = &[
    (
        "self.read_resource(&block)",
        "self.read_resource(&block, &block)",
        "mermaid-resource-backend:read",
    ),
    (
        "Err(error) => return (error, None),",
        "Err(other) => return (error, None),",
        "mermaid-resource-backend:read",
    ),
    (
        ".map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        ".map(|(output, extra)| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        "mermaid-resource-backend:entry",
    ),
    (
        "RenderOutputFactory::bind_resource(output, digest.as_deref())",
        "RenderOutputFactory::bind_resource(output, digest.as_ref())",
        "mermaid-resource-backend:entry",
    ),
    (
        "source: input.source.clone()",
        "source: input.source.clone(), kind: crate::markdown::DiagramKind::Mermaid",
        "mermaid-resource-backend:input",
    ),
    (
        "self.bundled.then_some(BundledResource::Mermaid)",
        "self.bundled.then_some(BundledResource::Mermaid, extra)",
        "mermaid-resource-backend:reader",
    ),
    (
        "(result, resource.into_digest())",
        "(result, resource.digest())",
        "mermaid-resource-backend:result",
    ),
    (
        "let preset = DiagramColorPreset::for_render_input(input);",
        "let _entry_shape = ();\n        let preset = DiagramColorPreset::for_render_input(input);",
        "mermaid-resource-backend:shape",
    ),
    (
        "DiagramColorPreset::for_render_input(input)",
        "OtherPreset::for_render_input(input)",
        "mermaid-resource-backend:callee:DiagramColorPreset::for_render_input",
    ),
    (
        "let (result, digest) = self.render_block(input);",
        "let _entry_context = ();\n        let (result, digest) = self.render_block(input);",
        "mermaid-resource-backend:entry",
    ),
    (
        "let (result, digest) = self.render_block(input);",
        "self.render_block(input);",
        "mermaid-resource-backend:entry",
    ),
    (
        "RenderOutputFactory::from_diagram_result(input, result, RuntimeDescriptor::mermaid())\n            .map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        "other",
        "mermaid-resource-backend:entry",
    ),
    (
        ".map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        ".and_then(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        "mermaid-resource-backend:entry",
    ),
    (
        "RenderOutputFactory::from_diagram_result(input, result, RuntimeDescriptor::mermaid())",
        "other",
        "mermaid-resource-backend:entry",
    ),
    (
        "RuntimeDescriptor::mermaid()",
        "RuntimeDescriptor::native()",
        "mermaid-resource-backend:entry",
    ),
    (
        "(result, resource.into_digest())",
        "result",
        "mermaid-resource-backend:result",
    ),
    (
        "DiagramBlock {\n            kind: crate::markdown::DiagramKind::Mermaid,\n            source: input.source.clone(),\n        }",
        "input.source.clone()",
        "mermaid-resource-backend:input",
    ),
    (
        "let resource = match self.read_resource(&block) {\n            Ok(resource) => resource,\n            Err(error) => return (error, None),\n        };",
        "let resource = self.read_resource(&block);",
        "mermaid-resource-backend:read",
    ),
    (
        "\n        runtime_resource_loader::load(",
        "\n        let _reader_context = ();\n        runtime_resource_loader::load(",
        "mermaid-resource-backend:reader",
    ),
];

pub(super) const INSTANCE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "svg.replace(&root_id, &unique_id)",
        "svg.replace(&root_id, &root_id)",
        "mermaid-resource-route:instance",
    ),
    (
        "MERMAID_SVG_RENDER_SEQUENCE.fetch_add(1, Ordering::Relaxed)",
        "MERMAID_SVG_RENDER_SEQUENCE.fetch_add(2, Ordering::Relaxed)",
        "mermaid-resource-route:instance",
    ),
];
