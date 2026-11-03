use super::{Fixture, INPUTS, mermaid_classic_scripts, reject_mutations};

const RESOURCE_BYTES_INDEX: usize = 9;
const RENDER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "runtime_resource_loader::load(",
        "runtime_resource_loader::other(",
        "mermaid-resource-route:load",
    ),
    (
        "bundled.then_some(BundledResource::Mermaid)",
        "bundled.then_some(BundledResource::Drawio)",
        "mermaid-resource-route:load",
    ),
    (
        "bundle.digest()",
        "other.digest()",
        "mermaid-resource-route:cache-digest",
    ),
    (
        "bundle.text()",
        "other.text()",
        "mermaid-resource-route:render",
    ),
    (
        "MermaidJsRuntimeOps::render_bundle(&block.source, bundle, preset)",
        "MermaidJsRuntimeOps::render_bundle(&block.source, other, preset)",
        "mermaid-resource-route:argument",
    ),
];

const LOADER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "let snapshot = ResourceBytes::snapshot(path)?;",
        "let snapshot = ResourceBytes::snapshot(other)?;",
        "resource-loader:source",
    ),
    (
        "ResourceSource::File { snapshot, text }",
        "ResourceSource::File { snapshot: other, text }",
        "resource-loader:source",
    ),
    (
        "Some(snapshot.digest())",
        "Some(other.digest())",
        "resource-loader:accessors",
    ),
    (
        "crate::resource_bytes::ResourceBytes::read_text(path)",
        "crate::resource_bytes::ResourceBytes::read_text(other)",
        "resource-loader:external-read",
    ),
    (
        "RuntimeAsset::mermaid()",
        "RuntimeAsset::drawio()",
        "mermaid-resource-route:bundled",
    ),
    (
        "Self::read_file(path).map(Cow::Owned)",
        "Self::read_file(other).map(Cow::Owned)",
        "resource-loader:external-role",
    ),
];

const BACKEND_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "self.render_empty(&block, &preset)",
        "self.render_empty(&other, &preset)",
        "mermaid-resource-backend:empty",
    ),
    (
        "MermaidRenderOps::render_mermaid_with_origin(\n            block,",
        "MermaidRenderOps::render_mermaid_with_origin(\n            other,",
        "mermaid-resource-backend:empty",
    ),
    (
        "self.render_block(input)",
        "self.render_block(other)",
        "mermaid-resource-backend:entry",
    ),
    (
        "RenderOutputFactory::bind_resource(output, digest.as_deref())",
        "RenderOutputFactory::bind_resource(output, other.as_deref())",
        "mermaid-resource-backend:entry",
    ),
    (
        "source: input.source.clone()",
        "source: other.source.clone()",
        "mermaid-resource-backend:input",
    ),
    (
        "self.read_resource(&block)",
        "self.read_resource(&other)",
        "mermaid-resource-backend:read",
    ),
    (
        "&resource,\n        );",
        "&other,\n        );",
        "mermaid-resource-backend:argument",
    ),
    (
        "(result, resource.into_digest())",
        "(result, other.into_digest())",
        "mermaid-resource-backend:result",
    ),
    (
        "runtime_resource_loader::load(\n            &self.runtime_path",
        "runtime_resource_loader::read(\n            &self.runtime_path",
        "mermaid-resource-backend:reader",
    ),
];

const INSTANCE_MUTATIONS: &[(&str, &str, &str)] = &[
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

const SNAPSHOT_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "File::open(path)",
        "File::open(other)",
        "resource-loader:snapshot",
    ),
    (
        "Sha256::digest(self.bytes())",
        "Sha256::digest(other.bytes())",
        "resource-loader:digest",
    ),
    (
        "String::from_utf8(bytes)",
        "String::from_utf8(other)",
        "resource-loader:content",
    ),
    (
        "snapshot.identity == *identity",
        "snapshot.identity != *identity",
        "resource-loader:cache",
    ),
];

#[test]
fn resource_route_rejects_loader_and_snapshot_drift() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mermaid_classic_inputs(INPUTS)?;
    mermaid_classic_scripts::inspect_root(fixture.root())?;
    reject_mutations(&fixture, INPUTS[5], RENDER_MUTATIONS)?;
    reject_mutations(&fixture, INPUTS[6], LOADER_MUTATIONS)?;
    reject_mutations(&fixture, INPUTS[7], BACKEND_MUTATIONS)?;
    reject_mutations(&fixture, INPUTS[8], INSTANCE_MUTATIONS)?;
    reject_mutations(&fixture, INPUTS[RESOURCE_BYTES_INDEX], SNAPSHOT_MUTATIONS)
}
