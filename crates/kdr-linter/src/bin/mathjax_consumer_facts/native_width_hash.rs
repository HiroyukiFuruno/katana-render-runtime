use super::source::{self, Source};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::ItemFn;

#[path = "native_width_hash_ast.rs"]
pub(super) mod ast;
#[cfg(test)]
#[path = "native_width_hash_import_tests.rs"]
mod import_tests;
#[path = "native_width_hash_routes.rs"]
mod routes;

pub(super) const HASH_INPUTS: &[&str] = &[
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/mod.rs",
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime.rs",
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/render.rs",
];
const HASH_FIELDS: &[&str] = &[
    "source",
    "preset.mermaid_theme",
    "preset.background",
    "preset.text",
    "preset.fill",
    "preset.stroke",
    "preset.arrow",
];
const CACHE_PREFIX: &[&str] = &[
    "MERMAID_RENDER_CACHE_SCHEMA",
    "MERMAID_RUNTIME_BUNDLE_CHECKSUMS",
    "zenuml_plugin_key",
    "mermaid_js",
    "mode.mermaid_cache_profile",
];
const CACHE_FILENAME_ARGUMENT_COUNT: usize = 3;
pub(super) fn run(root: &Path) -> Result<(), String> {
    let report = inspect_root(root)?;
    let output = report.to_string();
    println!("{output}");
    Ok(())
}

pub(super) fn inspect_root(root: &Path) -> Result<serde_json::Value, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("invalid-root:{error}"))?;
    let sources = read_sources(&root)?;
    require_hash_imports(&sources)?;
    routes::validate_hash_fields(&sources[HASH_INPUTS[0]])?;
    routes::validate_svg_id(&sources[HASH_INPUTS[1]])?;
    routes::validate_render_cache(&sources[HASH_INPUTS[2]])?;
    let hashes = sources
        .iter()
        .map(|(path, source)| ((*path).to_string(), source.sha256.clone()))
        .collect::<BTreeMap<_, _>>();
    Ok(json!({
        "schema": "kdr-linter/native-width-hash-facts/v1",
        "status": "verified",
        "facts": [
            {"id": "hash-fields", "path": HASH_INPUTS[0]},
            {"id": "svg-id-hash-route", "path": HASH_INPUTS[1]},
            {"id": "render-cache-hash-route", "path": HASH_INPUTS[2]}
        ],
        "inputSha256": hashes,
        "nativeExecutionVerified": false,
        "performanceVerified": false
    }))
}

fn require_hash_imports(sources: &BTreeMap<&'static str, Source>) -> Result<(), String> {
    let module = import_paths(&sources[HASH_INPUTS[0]].ast);
    if !module.contains("std::hash::Hash") || !module.contains("std::hash::Hasher") {
        return Err("hash-fields:imports".into());
    }
    for path in &HASH_INPUTS[1..] {
        let imports = import_paths(&sources[path].ast);
        if !imports.contains("std::collections::hash_map::DefaultHasher")
            || !imports.contains("std::hash::Hash")
            || !imports.contains("std::hash::Hasher")
        {
            return Err(format!("hash-route:imports:{path}"));
        }
    }
    Ok(())
}

fn import_paths(file: &syn::File) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for item in &file.items {
        if let syn::Item::Use(item) = item {
            collect_import_paths(&item.tree, &mut Vec::new(), &mut paths);
        }
    }
    paths
}

fn collect_import_paths(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    paths: &mut BTreeSet<String>,
) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_import_paths(&path.tree, prefix, paths);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            prefix.push(name.ident.to_string());
            paths.insert(prefix.join("::"));
            prefix.pop();
        }
        syn::UseTree::Rename(rename) => {
            prefix.push(rename.ident.to_string());
            paths.insert(prefix.join("::"));
            prefix.pop();
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_import_paths(item, prefix, paths);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

fn read_sources(root: &Path) -> Result<BTreeMap<&'static str, Source>, String> {
    HASH_INPUTS
        .iter()
        .map(|path| Ok((*path, source::source(root, path)?)))
        .collect()
}

fn require_hash_signature(function: &ItemFn) -> Result<(), String> {
    let parameters = function.sig.inputs.iter().collect::<Vec<_>>();
    let [hasher, source, preset] = parameters.as_slice() else {
        return Err("hash-fields:signature".into());
    };
    if !ast::named_reference(hasher, "hasher", "H", true)
        || !ast::named_reference(source, "source", "str", false)
        || !ast::named_reference(preset, "preset", "DiagramColorPreset", false)
        || function.sig.generics.params.len() != 1
    {
        return Err("hash-fields:signature".into());
    }
    let Some(syn::GenericParam::Type(parameter)) = function.sig.generics.params.first() else {
        return Err("hash-fields:signature".into());
    };
    if parameter.ident != "H"
        || parameter.bounds.len() != 1
        || !matches!(parameter.bounds.first(), Some(syn::TypeParamBound::Trait(bound)) if super::source::path_name(&bound.path) == "Hasher")
    {
        return Err("hash-fields:signature".into());
    }
    Ok(())
}
