use super::consumer::require_direct_call;
use super::source::{self, ident};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use syn::{Expr, Stmt, parse::Parser};

#[path = "mermaid_classic_scripts_includes.rs"]
mod includes;
#[path = "mermaid_classic_scripts_realm.rs"]
mod realm;
#[path = "mermaid_classic_scripts_route.rs"]
mod route;
#[path = "mermaid_classic_scripts_ast.rs"]
mod scripts;

pub(super) const INPUTS: &[&str] = &[
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime_scripts.rs",
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime.rs",
    "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs",
    "crates/katana-render-runtime/src/v8_script.rs",
    "crates/katana-render-runtime/src/lib.rs",
    "Cargo.toml",
    "Cargo.lock",
    "crates/kdr-linter/Cargo.toml",
];
const RUST_INPUT_COUNT: usize = 5;
const SCRIPT_INPUT_INDEX: usize = 0;
const CALLER_INPUT_INDEX: usize = 1;
const REALM_INPUT_INDEX: usize = 2;
const SCHEMA: &str = "kdr-linter/mermaid-classic-script-facts/v1";

pub(super) fn run(root: &Path) -> Result<(), String> {
    let output = match inspect_root(root) {
        Ok(value) => value,
        Err(error) => json!({"schema": SCHEMA, "status": "rejected", "errors": [error]}),
    };
    println!(
        "{}",
        serde_json::to_string(&output).map_err(|error| error.to_string())?
    );
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
    route::inspect_caller(&sources[INPUTS[CALLER_INPUT_INDEX]], &mut facts)?;
    realm::inspect_realm(&sources[INPUTS[REALM_INPUT_INDEX]], &mut facts)?;
    super::v8_route::inspect_v8_compile(&sources, &mut facts)?;
    super::sort_facts(&mut facts);
    let hashes = INPUTS
        .iter()
        .map(|path| Ok((*path, source::opaque_sha256(&root, path)?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    for (path, source) in &sources {
        if hashes.get(path) != Some(&source.sha256) {
            return Err(format!("mermaid-classic:source-drift:{path}"));
        }
    }
    Ok(static_report(parser, facts, hashes, manifest))
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

fn arguments(tokens: &proc_macro2::TokenStream) -> Result<Vec<Expr>, String> {
    Ok(
        syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
            .parse2(tokens.clone())
            .map_err(|_| "mermaid-scripts:macro-arguments")?
            .into_iter()
            .collect(),
    )
}

fn literal(expression: &Expr) -> Result<String, String> {
    match expression {
        Expr::Lit(value) if matches!(value.lit, syn::Lit::Str(_)) => {
            let syn::Lit::Str(text) = &value.lit else {
                return Err("mermaid-scripts:literal".into());
            };
            Ok(text.value())
        }
        _ => Err("mermaid-scripts:literal".into()),
    }
}

fn script_entry(expression: &Expr, name: &str, code: &str, owned: bool) -> Result<Value, String> {
    let Expr::Call(call) = expression else {
        return Err("mermaid-scripts:entry".into());
    };
    let constructor = if owned {
        "DiagramRuntimeScript::owned"
    } else {
        "DiagramRuntimeScript::borrowed"
    };
    if !ident(&call.func, constructor) || call.args.len() != 2 || literal(&call.args[0])? != name {
        return Err("mermaid-scripts:entry".into());
    }
    if owned {
        require_direct_call(&call.args[1], "render_script", &[code], "mermaid-scripts")?;
    } else if !ident(&call.args[1], code) {
        return Err("mermaid-scripts:entry-code".into());
    }
    Ok(
        json!({"name": literal(&call.args[0])?, "ownership": if owned { "owned" } else { "borrowed" },
        "sourceBinding": code, "producer": if owned { "render_script" } else { "identity" }}),
    )
}

fn push_entry(
    statement: Option<&Stmt>,
    name: &str,
    code: &str,
    owned: bool,
) -> Result<Value, String> {
    let Some(Stmt::Expr(Expr::MethodCall(push), Some(_))) = statement else {
        return Err("mermaid-scripts:push".into());
    };
    if push.method != "push" || !ident(&push.receiver, "scripts") || push.args.len() != 1 {
        return Err("mermaid-scripts:push".into());
    }
    script_entry(&push.args[0], name, code, owned)
}

fn macro_statement(statement: Option<&Stmt>, expected: &str) -> Result<(), String> {
    let Some(Stmt::Macro(actual)) = statement else {
        return Err("mermaid-classic-realm:scope-macro".into());
    };
    let Stmt::Macro(expected) =
        syn::parse_str::<Stmt>(expected).map_err(|_| "mermaid-classic-realm:scope-template")?
    else {
        return Err("mermaid-classic-realm:scope-template".into());
    };
    if source::path_name(&actual.mac.path) != source::path_name(&expected.mac.path)
        || actual.mac.tokens.to_string() != expected.mac.tokens.to_string()
        || actual.semi_token.is_none()
    {
        return Err("mermaid-classic-realm:scope-macro".into());
    }
    Ok(())
}

fn tail(block: &syn::Block) -> Result<&Expr, String> {
    match block.stmts.last() {
        Some(Stmt::Expr(tail, None)) => Ok(tail),
        _ => Err("mermaid-classic-route:tail".into()),
    }
}

fn plain_local(block: &syn::Block, index: usize, name: &str) -> Result<(), String> {
    if !matches!(block.stmts.get(index), Some(Stmt::Local(local)) if matches!(&local.pat, syn::Pat::Ident(binding) if binding.ident == name && binding.subpat.is_none() && binding.by_ref.is_none()) && local.init.as_ref().is_some_and(|init| init.diverge.is_none()))
    {
        return Err(format!("mermaid-request-route:binding:{name}"));
    }
    Ok(())
}
