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
