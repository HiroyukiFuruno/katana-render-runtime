use super::super::mathjax_facts_ast_linter_tests::shared_resource_loader_mutations::SHARED_RESOURCE_LOADER_MUTATIONS;
use super::{Fixture, INPUTS, mermaid_classic_scripts, reject_mutations};

const RESOURCE_BYTES_INDEX: usize = 9;
const RENDER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "Ok(bundle) => bundle,",
        "Ok(bundle) => other,",
        "mermaid-resource-route:load",
    ),
    (
        "Self::error(block, format!(\"Failed to read Mermaid.js bundle: {error}\"))",
        "Self::error(other, format!(\"Failed to read Mermaid.js bundle: {error}\"))",
        "mermaid-resource-route:load",
    ),
    (
        "let mut cache_file = Self::cache_file_path",
        "let cache_file = Self::cache_file_path",
        "mermaid-resource-route:cache-binding",
    ),
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
        "cache_file.set_extension(format!(\"{digest}.{}\", mode.mermaid_cache_extension()));",
        "other.set_extension(format!(\"{digest}.{}\", mode.mermaid_cache_extension()));",
        "mermaid-resource-route:cache-digest",
    ),
    (
        "Self::error(block, format!(\"Failed to read Mermaid.js bundle: {error}\"))",
        "Other::error(block, format!(\"Failed to read Mermaid.js bundle: {error}\"))",
        "mermaid-resource-route:load",
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
        "RuntimeAsset::mermaid()",
        "RuntimeAsset::drawio()",
        "mermaid-resource-route:bundled",
    ),
    (
        "Some(BundledResource::Mermaid) => RuntimeAsset::mermaid()",
        "Some(BundledResource::Drawio) => RuntimeAsset::mermaid()",
        "mermaid-resource-route:bundled",
    ),
];

const BACKEND_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "Err(error) => return (error, None),",
        "Err(error) => return (other, None),",
        "mermaid-resource-backend:read",
    ),
    (
        "Ok(resource) => resource,",
        "Ok(resource) => other,",
        "mermaid-resource-backend:read",
    ),
    (
        "self.bundled.then_some(BundledResource::Mermaid)",
        "self.bundled.then_some(BundledResource::Drawio)",
        "mermaid-resource-backend:reader",
    ),
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
        "let (result, digest) = self.render_block(input);",
        "let (result, other) = self.render_block(input);",
        "mermaid-resource-backend:entry",
    ),
    (
        ".map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        ".map(|other| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
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
    reject_mutations(&fixture, INPUTS[6], SHARED_RESOURCE_LOADER_MUTATIONS)?;
    reject_mutations(&fixture, INPUTS[6], LOADER_MUTATIONS)?;
    reject_mutations(&fixture, INPUTS[7], BACKEND_MUTATIONS)?;
    reject_mutations(&fixture, INPUTS[8], INSTANCE_MUTATIONS)?;
    reject_mutations(&fixture, INPUTS[RESOURCE_BYTES_INDEX], SNAPSHOT_MUTATIONS)
}
