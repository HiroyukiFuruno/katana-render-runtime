use super::super::source::{Source, free_function, ident, local, method, path_name, record_fact};
use super::{
    INPUTS, SCRIPT_INPUT_INDEX, arguments, includes, literal, plain_local, push_entry, script_entry,
};
use serde_json::{Value, json};
use syn::{Expr, Stmt, spanned::Spanned};

type ScriptManifests = (Vec<Value>, Vec<Value>);

const BUILD_FORWARD_ARGUMENTS: usize = 4;
const BUILD_FORWARD_BASE_ARGUMENTS: usize = 3;
const BUILD_SCRIPTS_STATEMENTS: usize = 4;
const BASE_SCRIPT_ENTRIES: usize = 3;
const SCRIPT_PUSH_STATEMENT_INDEX: usize = 2;
#[cfg(test)]
#[path = "mermaid_classic_scripts_ast_unit_tests.rs"]
mod unit_tests;

pub(super) fn inspect_scripts(source: &Source, facts: &mut Vec<Value>) -> Result<Value, String> {
    let path = INPUTS[SCRIPT_INPUT_INDEX];
    let (build, scripts) = builders(source)?;
    let (includes, include_span) = includes::inspect(source)?;
    let (normal, zenuml) = script_manifests(&scripts.block)?;
    let format = render_format(source)?;
    for (id, span) in [
        ("mermaid-build-forwarding", build.span()),
        ("mermaid-classic-source-identities", include_span),
        ("mermaid-classic-script-order", scripts.span()),
        ("mermaid-render-format", format.1),
    ] {
        record_fact(facts, id, path, span);
    }
    Ok(
        json!({"normal": normal, "zenuml": zenuml, "sourceIdentities": includes,
        "formatProducer": {"semanticTemplate": format.0, "entrypoint": "katanaRunMermaidRuntime",
        "interpolation": "request_json", "interpolationRole": "JSON data; actual bytes unavailable"},
        "scriptKind": "individual classic scripts in one V8 realm"}),
    )
}

fn builders(source: &Source) -> Result<(&syn::ImplItemFn, &syn::ImplItemFn), String> {
    let build = method(&source.ast, "MermaidRuntimeScripts", "build")?;
    let scripts = method(&source.ast, "MermaidRuntimeScripts", "build_scripts")?;
    parameters(&build.sig, &["bundle", "request_json", "diagram_type"])?;
    parameters(
        &scripts.sig,
        &[
            "bundle",
            "zenuml_bundle",
            "request_json",
            "uses_zenuml_runtime",
        ],
    )?;
    if build.block.stmts.len() != 1 || scripts.block.stmts.len() != BUILD_SCRIPTS_STATEMENTS {
        return Err("mermaid-scripts:body".into());
    }
    forwarding(build)?;
    Ok((build, scripts))
}

fn forwarding(build: &syn::ImplItemFn) -> Result<(), String> {
    let Some(Stmt::Expr(Expr::Call(forward), None)) = build.block.stmts.first() else {
        return Err("mermaid-scripts:forward".into());
    };
    if !ident(&forward.func, "Self::build_scripts")
        || forward.args.len() != BUILD_FORWARD_ARGUMENTS
        || !forward
            .args
            .iter()
            .take(BUILD_FORWARD_BASE_ARGUMENTS)
            .zip(["bundle", "MERMAID_ZENUML_RUNTIME", "request_json"])
            .all(|(arg, name)| ident(arg, name))
        || !matches!(&forward.args[3], Expr::Binary(test) if matches!(test.op, syn::BinOp::Eq(_)) && ident(&test.left, "diagram_type") && ident(&test.right, "MermaidDiagramType::Zenuml"))
    {
        return Err("mermaid-scripts:forward".into());
    }
    Ok(())
}

fn base_scripts(block: &syn::Block) -> Result<Vec<Value>, String> {
    plain_local(block, 0, "scripts")?;
    if !matches!(block.stmts.first(), Some(Stmt::Local(local)) if matches!(&local.pat, syn::Pat::Ident(binding) if binding.mutability.is_some()))
    {
        return Err("mermaid-scripts:vector-binding".into());
    }
    let vector = match local(block, 0, "scripts")? {
        Expr::Macro(vector) if path_name(&vector.mac.path) == "vec" => vector,
        _ => return Err("mermaid-scripts:vector".into()),
    };
    let entries = arguments(&vector.mac.tokens)?;
    if entries.len() != BASE_SCRIPT_ENTRIES {
        return Err("mermaid-scripts:base-count".into());
    }
    entries
        .iter()
        .zip([
            ("mermaid-runtime.min.js", "MERMAID_RUNTIME"),
            ("mermaid.min.js", "bundle"),
            ("mermaid-zenuml.min.js", "zenuml_bundle"),
        ])
        .map(|(entry, (name, code))| script_entry(entry, name, code, false))
        .collect()
}

fn zenuml_script(block: &syn::Block) -> Result<Value, String> {
    let Some(Stmt::Expr(Expr::If(branch), None)) = block.stmts.get(1) else {
        return Err("mermaid-scripts:zen-branch".into());
    };
    if !ident(&branch.cond, "uses_zenuml_runtime")
        || branch.else_branch.is_some()
        || branch.then_branch.stmts.len() != 1
    {
        return Err("mermaid-scripts:zen-branch".into());
    }
    push_entry(
        branch.then_branch.stmts.first(),
        "zenuml-runtime.min.js",
        "ZENUML_RUNTIME",
        false,
    )
}

fn script_manifests(block: &syn::Block) -> Result<ScriptManifests, String> {
    let mut normal = base_scripts(block)?;
    let zen = zenuml_script(block)?;
    let entry = push_entry(
        block.stmts.get(SCRIPT_PUSH_STATEMENT_INDEX),
        "render-mermaid.js",
        "request_json",
        true,
    )?;
    if !matches!(block.stmts.last(), Some(Stmt::Expr(tail, None)) if ident(tail, "scripts")) {
        return Err("mermaid-scripts:tail".into());
    }
    let mut zenuml = normal.clone();
    zenuml.push(zen);
    normal.push(entry.clone());
    zenuml.push(entry);
    Ok((normal, zenuml))
}
pub(super) fn parameters(sig: &syn::Signature, names: &[&str]) -> Result<(), String> {
    if sig.inputs.len() != names.len() || !sig.inputs.iter().zip(names).all(|(arg, name)|
        matches!(arg, syn::FnArg::Typed(arg) if matches!(arg.pat.as_ref(), syn::Pat::Ident(binding) if binding.ident == *name && binding.subpat.is_none() && binding.by_ref.is_none()))) {
        return Err("mermaid-scripts:parameters".into());
    }
    Ok(())
}
fn render_format(source: &Source) -> Result<(String, proc_macro2::Span), String> {
    let render = free_function(&source.ast, "render_script")?;
    parameters(&render.sig, &["request_json"])?;
    let [Stmt::Expr(Expr::Macro(formatted), None)] = render.block.stmts.as_slice() else {
        return Err("mermaid-scripts:format".into());
    };
    let args = arguments(&formatted.mac.tokens)?;
    if path_name(&formatted.mac.path) != "format"
        || args.len() != 1
        || literal(&args[0])? != "katanaRunMermaidRuntime({request_json});"
    {
        return Err("mermaid-scripts:format".into());
    }
    Ok((literal(&args[0])?, args[0].span()))
}
