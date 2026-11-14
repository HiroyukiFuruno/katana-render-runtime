use super::source;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

#[path = "mermaid_classic_scripts_helpers.rs"]
mod helpers;
#[path = "mermaid_classic_scripts_includes.rs"]
mod includes;
#[path = "mermaid_classic_scripts_realm.rs"]
mod realm;
#[path = "mermaid_classic_resource_backend.rs"]
mod resource_backend;
#[path = "mermaid_classic_resource_loader.rs"]
mod resource_loader;
#[path = "mermaid_classic_resource_route.rs"]
mod resource_route;
#[path = "mermaid_classic_scripts_route.rs"]
mod route;
#[path = "mermaid_classic_scripts_ast.rs"]
mod scripts;
#[cfg(test)]
#[path = "mermaid_classic_scripts_unit_tests.rs"]
mod unit_tests;

pub(super) use helpers::{
    arguments, literal, macro_statement, plain_local, push_entry, script_entry, tail,
};

pub(super) const INPUTS: &[&str] = &[
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime_scripts.rs",
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime.rs",
    "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs",
    "crates/katana-render-runtime/src/v8_script.rs",
    "crates/katana-render-runtime/src/lib.rs",
    source::RUST_INPUTS[10],
    source::RUST_INPUTS[9],
    "crates/katana-render-runtime/src/renderer/backends/mermaid.rs",
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/render_instance.rs",
    source::RUST_INPUTS[source::RESOURCE_BYTES_INPUT_INDEX],
    "Cargo.toml",
    "Cargo.lock",
    "crates/kdr-linter/Cargo.toml",
];
const RUST_INPUT_COUNT: usize = 10;
const SCRIPT_INPUT_INDEX: usize = 0;
const CALLER_INPUT_INDEX: usize = 1;
const REALM_INPUT_INDEX: usize = 2;
const RENDER_INPUT_INDEX: usize = 5;
const RESOURCE_LOADER_INPUT_INDEX: usize = 6;
const BACKEND_INPUT_INDEX: usize = 7;
const INSTANCE_INPUT_INDEX: usize = 8;
const SCHEMA: &str = "kdr-linter/mermaid-classic-script-facts/v1";

pub(super) fn run(root: &Path) -> Result<(), String> {
    let output = match inspect_root(root) {
        Ok(value) => value,
        Err(error) => json!({"schema": SCHEMA, "status": "rejected", "errors": [error]}),
    };
    let serialized = output.to_string();
    println!("{serialized}");
    if output["status"] != "verified" {
        return Err("Mermaid classic source route rejected".into());
    }
    Ok(())
}

pub(super) fn inspect_root(root: &Path) -> Result<Value, String> {
    let root = super::canonical_root(root)?;
    let parser = source::parser_pin(
        &root,
        super::BUILD_CARGO_LOCK,
        super::BUILD_WORKSPACE_MANIFEST,
        super::BUILD_LINTER_MANIFEST,
    )?;
    let sources = INPUTS[..RUST_INPUT_COUNT]
        .iter()
        .map(|path| Ok((*path, source::source(&root, path)?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let mut facts = Vec::new();
    let manifest = scripts::inspect_scripts(&sources[INPUTS[SCRIPT_INPUT_INDEX]], &mut facts)?;
    inspect_routes(&sources, &mut facts)?;
    super::sort_facts(&mut facts);
    let hashes = INPUTS
        .iter()
        .map(|path| Ok((*path, source::opaque_sha256(&root, path)?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let before = sources
        .iter()
        .map(|(path, source)| (*path, source.sha256.clone()))
        .collect();
    verify_source_snapshots(&before, &hashes)?;
    Ok(static_report(parser, facts, hashes, manifest))
}

fn inspect_routes(
    sources: &BTreeMap<&str, source::Source>,
    facts: &mut Vec<Value>,
) -> Result<(), String> {
    route::inspect_caller(&sources[INPUTS[CALLER_INPUT_INDEX]], facts)?;
    resource_route::inspect(
        &sources[INPUTS[RENDER_INPUT_INDEX]],
        &sources[INPUTS[RESOURCE_LOADER_INPUT_INDEX]],
    )?;
    resource_backend::inspect(&sources[INPUTS[BACKEND_INPUT_INDEX]])?;
    super::call_binding::ResourceLoaderGuard::require_bytes(
        &sources[source::RUST_INPUTS[source::RESOURCE_BYTES_INPUT_INDEX]].ast,
    )?;
    resource_route::InstanceGuard::inspect(
        &sources[INPUTS[RENDER_INPUT_INDEX]],
        &sources[INPUTS[INSTANCE_INPUT_INDEX]],
    )?;
    realm::inspect_realm(&sources[INPUTS[REALM_INPUT_INDEX]], facts)?;
    super::v8_route::inspect_v8_compile(sources, facts)
}

fn verify_source_snapshots(
    before: &BTreeMap<&str, String>,
    after: &BTreeMap<&str, String>,
) -> Result<(), String> {
    for (path, sha256) in before {
        if after.get(path) != Some(sha256) {
            return Err(format!("mermaid-classic:source-drift:{path}"));
        }
    }
    Ok(())
}

fn static_report(
    parser: String,
    facts: Vec<Value>,
    hashes: BTreeMap<&str, String>,
    manifest: Value,
) -> Value {
    json!({"schema": SCHEMA, "status": "verified", "parser": {
        "crate": "syn", "resolvedVersion": parser, "features": ["full", "visit"], "spanLocations": true},
        "facts": facts, "inputSha256": hashes, "manifest": manifest, "sourceProofVerified": true,
        "requestBytesVerified": false, "materializedScriptsVerified": false,
        "vendorBytesVerified": false, "pluginBytesVerified": false,
        "nativeExecutionVerified": false, "performanceVerified": false, "acceptedConsumerClosure": false})
}
