#[path = "mathjax_consumer_facts/ast_lookup.rs"]
mod ast_lookup;
#[path = "mathjax_consumer_facts/bundle.rs"]
mod bundle;
#[path = "mathjax_consumer_facts/call_binding.rs"]
mod call_binding;
#[path = "mathjax_consumer_facts/consumer.rs"]
mod consumer;
#[path = "mathjax_consumer_facts/default_materialize.rs"]
mod default_materialize;
#[path = "mathjax_consumer_facts/default_route.rs"]
mod default_route;
#[path = "mathjax_consumer_facts/macho_noso.rs"]
mod macho_noso;
#[path = "mathjax_consumer_facts/macho_noso_binary.rs"]
mod macho_noso_binary;
#[path = "mathjax_consumer_facts/macho_noso_parser_pin.rs"]
mod macho_noso_parser_pin;
#[path = "mathjax_consumer_facts/materializer.rs"]
mod materializer;
#[path = "mathjax_consumer_facts/materializer_body.rs"]
mod materializer_body;
#[path = "mathjax_consumer_facts/materializer_readback.rs"]
mod materializer_readback;
#[path = "mathjax_consumer_facts/mermaid_classic_scripts.rs"]
mod mermaid_classic_scripts;
#[path = "mathjax_consumer_facts/native_width_hash.rs"]
mod native_width_hash;
#[path = "mathjax_consumer_facts/renderer_trait.rs"]
mod renderer_trait;
#[path = "mathjax_consumer_facts/runtime_dispatch.rs"]
mod runtime_dispatch;
#[path = "mathjax_consumer_facts/source.rs"]
mod source;
#[path = "mathjax_consumer_facts/syntax.rs"]
mod syntax;
#[cfg(test)]
#[path = "mathjax_consumer_facts/syntax_tests.rs"]
mod syntax_tests;
#[path = "mathjax_consumer_facts/v8_compile.rs"]
mod v8_compile;
#[path = "mathjax_consumer_facts/v8_route.rs"]
mod v8_route;

use serde_json::json;
use source::{OPAQUE_INPUTS, RUST_INPUTS};
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

const BUILD_CARGO_LOCK: &str = include_str!("../../../../Cargo.lock");
const BUILD_WORKSPACE_MANIFEST: &str = include_str!("../../../../Cargo.toml");
const BUILD_LINTER_MANIFEST: &str = include_str!("../../Cargo.toml");

fn inspect_root(root: &Path) -> Result<serde_json::Value, String> {
    let root = canonical_root(root)?;
    let sources = read_sources(&root)?;
    let parser_version = source::parser_pin(
        &root,
        BUILD_CARGO_LOCK,
        BUILD_WORKSPACE_MANIFEST,
        BUILD_LINTER_MANIFEST,
    )?;
    let facts = inspect_facts(&sources)?;
    let hashes = input_hashes(&root, &sources)?;
    Ok(static_report(parser_version, facts, hashes))
}

fn canonical_root(root: &Path) -> Result<PathBuf, String> {
    root.canonicalize()
        .map_err(|error| format!("invalid-root:{error}"))
}

fn read_sources(root: &Path) -> Result<BTreeMap<&'static str, source::Source>, String> {
    let mut sources = BTreeMap::new();
    for path in RUST_INPUTS {
        sources.insert(*path, source::source(root, path)?);
    }
    Ok(sources)
}

fn inspect_facts(
    sources: &BTreeMap<&'static str, source::Source>,
) -> Result<Vec<serde_json::Value>, String> {
    let mut facts = Vec::new();
    bundle::inspect_scripts(&sources[RUST_INPUTS[2]], &mut facts)?;
    consumer::inspect_callers(sources, &mut facts)?;
    default_route::inspect_overrides(sources, &mut facts)?;
    sort_facts(&mut facts);
    Ok(facts)
}

fn input_hashes(
    root: &Path,
    sources: &BTreeMap<&'static str, source::Source>,
) -> Result<BTreeMap<&'static str, String>, String> {
    let mut hashes = BTreeMap::new();
    for (path, source) in sources {
        hashes.insert(*path, source.sha256.clone());
    }
    for path in OPAQUE_INPUTS {
        hashes.insert(*path, source::opaque_sha256(root, path)?);
    }
    Ok(hashes)
}

fn sort_facts(facts: &mut [serde_json::Value]) {
    facts.sort_by(|left, right| {
        left["id"]
            .as_str()
            .cmp(&right["id"].as_str())
            .then(left["path"].as_str().cmp(&right["path"].as_str()))
    });
}

fn static_report(
    parser_version: String,
    facts: Vec<serde_json::Value>,
    hashes: BTreeMap<&'static str, String>,
) -> serde_json::Value {
    json!({
        "schema": "kdr-linter/mathjax-static-facts/v1",
        "status": "verified",
        "parser": {"crate": "syn", "resolvedVersion": parser_version, "features": ["full", "visit"], "spanLocations": true},
        "facts": facts,
        "inputSha256": hashes,
        "generatedGlobalVerified": false,
        "overrideContentVerified": false
    })
}

fn run() -> Result<(), String> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    if let [mode, root] = args.as_slice()
        && mode == "--mermaid-classic-scripts"
    {
        return mermaid_classic_scripts::run(&PathBuf::from(root));
    }
    if let [mode, root] = args.as_slice()
        && mode == "--native-width-hash"
    {
        return native_width_hash::run(&PathBuf::from(root));
    }
    if let [mode, manifest] = args.as_slice()
        && mode == "--macho-noso"
    {
        return macho_noso::MachoNoso::run(&PathBuf::from(manifest));
    }
    let [root] = args.as_slice() else {
        return Err("usage: mathjax_consumer_facts <repo-root>".into());
    };
    let root = PathBuf::from(root);
    print_report(&root)
}

fn print_report(root: &Path) -> Result<(), String> {
    let output = match inspect_root(root) {
        Ok(value) => value,
        Err(error) => json!({
            "schema": "kdr-linter/mathjax-static-facts/v1",
            "status": "rejected",
            "errors": [error]
        }),
    };
    println!(
        "{}",
        serde_json::to_string(&output).map_err(|error| error.to_string())?
    );
    if output["status"] != "verified" {
        return Err("MathJax static route rejected".into());
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
#[path = "mathjax_consumer_facts/macho_noso_test_support.rs"]
mod macho_noso_test_support;
#[cfg(test)]
#[path = "mathjax_consumer_facts/macho_noso_tests.rs"]
mod macho_noso_tests;
#[cfg(test)]
#[path = "mathjax_consumer_facts/ast_linter_tests.rs"]
mod mathjax_facts_ast_linter_tests;
#[cfg(test)]
#[path = "mathjax_consumer_facts/mermaid_classic_scripts_tests.rs"]
mod mermaid_classic_scripts_ast_linter_tests;
#[cfg(test)]
#[path = "mathjax_consumer_facts/native_width_hash_tests.rs"]
mod native_width_hash_ast_linter_tests;
#[cfg(test)]
#[path = "mathjax_consumer_facts/opaque_hash_tests.rs"]
mod opaque_hash_tests;
#[cfg(test)]
#[path = "mathjax_consumer_facts/test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "mathjax_consumer_facts/test_support_verdict_tests.rs"]
mod test_support_verdict_tests;
