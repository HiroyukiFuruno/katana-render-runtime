use super::super::mathjax_facts_ast_linter_tests::shared_resource_loader_mutations::SHARED_RESOURCE_LOADER_MUTATIONS;
use super::{Fixture, INPUTS, mermaid_classic_scripts, reject_mutations};

const RESOURCE_BYTES_INDEX: usize = 9;
const RENDER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "Err(error) => {\n                return Self::error(block, format!(\"Failed to read Mermaid.js bundle: {error}\"));\n            }",
        "Err(error) => other,",
        "mermaid-resource-route:load",
    ),
    (
        "return Self::error(block, format!(\"Failed to read Mermaid.js bundle: {error}\"));",
        "return other;",
        "mermaid-resource-route:load",
    ),
    (
        "Self::render_svg_with_bundle(block, bundle.text(), preset, &cache_file)",
        "other",
        "mermaid-resource-route:render",
    ),
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
        "Some(BundledResource::Mermaid) => RuntimeAsset::mermaid()\n                .bytes()\n                .map_err(std::io::Error::other)?,",
        "Some(BundledResource::Mermaid) => RuntimeAsset::mermaid()\n                .bytes()\n                .map_err(std::io::Error::other),",
        "mermaid-resource-route:bundled",
    ),
    (
        "let source = match bundled {",
        "let source = match other {",
        "resource-loader:read-dispatch",
    ),
    (
        "None => return Self::read_file(path).map(Cow::Owned)",
        "ref None => return Self::read_file(path).map(Cow::Owned)",
        "mermaid-resource-route:external",
    ),
    (
        "Self::read_file(path).map(Cow::Owned)",
        "(Self::read_file)(path).map(Cow::Owned)",
        "mermaid-resource-route:callee:Self::read_file",
    ),
    (
        ".map(Cow::Borrowed)",
        ".map(Cow::Owned)",
        "mermaid-resource-route:utf8",
    ),
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
        "runtime_resource_loader::load(\n            &self.runtime_path,\n            self.bundled.then_some(BundledResource::Mermaid),\n        )\n        .map_err(|error| {\n            runtime_resource_loader::diagram_error(\n                block,\n                &self.runtime_path,\n                error,\n                \"Mermaid\",\n                crate::markdown::runtime_assets::MERMAID_DOWNLOAD_URL,\n                \"Failed to read Mermaid.js bundle\",\n            )\n        })",
        "other",
        "mermaid-resource-backend:reader",
    ),
    (
        "runtime_resource_loader::load(\n            &self.runtime_path,\n            self.bundled.then_some(BundledResource::Mermaid),\n        )\n        .map_err(|error| {",
        "other.map_err(|error| {",
        "mermaid-resource-backend:reader",
    ),
    (
        ".map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        ".map(other)",
        "mermaid-resource-backend:entry",
    ),
    (
        ".map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        ".map(|output| output)",
        "mermaid-resource-backend:entry",
    ),
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

type Replacement = (&'static str, &'static str);

struct LoaderPriorityCase {
    replacements: [Replacement; 2],
    expected: &'static str,
}

const WRONG_ASSET: Replacement = ("RuntimeAsset::mermaid()", "RuntimeAsset::drawio()");
const TERMINATED_TAIL: Replacement = (
    ".map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))",
    ".map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error));",
);
const LOADER_PRIORITY_CASES: &[LoaderPriorityCase] = &[
    LoaderPriorityCase {
        replacements: [
            ("let source = match bundled {", "let source = match other {"),
            WRONG_ASSET,
        ],
        expected: "resource-loader:read-dispatch",
    },
    LoaderPriorityCase {
        replacements: [WRONG_ASSET, TERMINATED_TAIL],
        expected: "mermaid-resource-route:bundled",
    },
    LoaderPriorityCase {
        replacements: [
            (
                "None => return Self::read_file(path).map(Cow::Owned)",
                "ref None => return Self::read_file(path).map(Cow::Owned)",
            ),
            TERMINATED_TAIL,
        ],
        expected: "mermaid-classic-route:tail",
    },
];

#[test]
fn resource_loader_preserves_multi_error_priority() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mermaid_classic_inputs(INPUTS)?;
    for case in LOADER_PRIORITY_CASES {
        for (original, changed) in case.replacements {
            fixture.replace(INPUTS[6], original, changed)?;
        }
        fixture.reject(case.expected)?;
        fixture.restore(INPUTS[6])?;
        mermaid_classic_scripts::inspect_root(fixture.root())?;
    }
    Ok(())
}
