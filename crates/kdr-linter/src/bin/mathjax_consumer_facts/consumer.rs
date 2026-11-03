use super::call_binding::require_request_json;
use super::source::{RUST_INPUTS, Source, call, ident, method, record_fact};
use serde_json::Value;
use std::collections::BTreeMap;
use syn::spanned::Spanned;

#[path = "consumer_loaded_route.rs"]
mod loaded_route;

const RESOURCE_LOADER_INPUT_INDEX: usize = 9;
const RENDER_STATEMENTS: usize = 5;
const SCRIPTS_BUILD_STATEMENT_INDEX: usize = 2;
const RENDER_CALL_STATEMENT_INDEX: usize = 3;
const RESPONSE_MATCH_STATEMENT_INDEX: usize = 4;

pub(super) fn inspect_callers(
    sources: &BTreeMap<&str, Source>,
    facts: &mut Vec<Value>,
) -> Result<(), String> {
    inspect_mathjax_caller(sources, facts)?;
    super::v8_route::inspect_v8_compile(sources, facts)
}

fn inspect_mathjax_caller(
    sources: &BTreeMap<&str, Source>,
    facts: &mut Vec<Value>,
) -> Result<(), String> {
    let path = RUST_INPUTS[1];
    let render = method(&sources[path].ast, "MathJaxJsRuntimeOps", "render_bundle")?;
    let statements = &render.block.stmts;
    if statements.len() != RENDER_STATEMENTS {
        return Err("read-render-route:statement-count".into());
    }
    loaded_route::require_source_input(&sources[RUST_INPUTS[0]].ast, &sources[path].ast)?;
    super::call_binding::ResourceLoaderGuard::require_resource_loader(
        &sources[RUST_INPUTS[RESOURCE_LOADER_INPUT_INDEX]].ast,
    )?;
    super::call_binding::ResourceLoaderGuard::require_bytes(
        &sources[RUST_INPUTS[super::source::RESOURCE_BYTES_INPUT_INDEX]].ast,
    )?;
    require_request_input(&render.block)?;
    require_script_build(&render.block)?;
    require_output_render(&render.block)?;
    require_response_match(&statements[RESPONSE_MATCH_STATEMENT_INDEX])?;
    record_fact(facts, "read-render-route", path, render.span());
    Ok(())
}

fn require_request_input(block: &syn::Block) -> Result<(), String> {
    require_local_call(block, 0, "request", "MathJaxRenderRequest::new")?;
    require_args(
        local(block, 0)?,
        &["source", "preset", "display"],
        "read-render-route",
    )?;
    require_request_json(local(block, 1)?)
}

fn require_script_build(block: &syn::Block) -> Result<(), String> {
    require_local_call(
        block,
        SCRIPTS_BUILD_STATEMENT_INDEX,
        "scripts",
        "MathJaxRuntimeScripts::build",
    )?;
    require_args(
        local(block, SCRIPTS_BUILD_STATEMENT_INDEX)?,
        &["mathjax_source", "request_json"],
        "read-render-route",
    )
}

fn require_output_render(block: &syn::Block) -> Result<(), String> {
    require_local_call(
        block,
        RENDER_CALL_STATEMENT_INDEX,
        "output",
        "DiagramV8Runtime::render",
    )?;
    require_call_arg(
        local(block, RENDER_CALL_STATEMENT_INDEX)?,
        0,
        "scripts",
        "read-render-route",
    )
}

fn require_response_match(statement: &syn::Stmt) -> Result<(), String> {
    let syn::Stmt::Expr(syn::Expr::Match(response), _) = statement else {
        return Err("read-render-route:response-match".into());
    };
    require_direct_call(
        &response.expr,
        "parse_response",
        &["output"],
        "read-render-route",
    )?;
    require_response_arms(response)
}

fn require_response_arms(expression: &syn::ExprMatch) -> Result<(), String> {
    if expression.arms.len() != 2 {
        return Err("read-render-route:response-arms".into());
    }
    let arms = expression.arms.iter().collect::<Vec<_>>();
    let [svg, error] = arms.as_slice() else {
        return Err("read-render-route:response-arms".into());
    };
    require_pattern_field(&svg.pat, "Svg", "svg")?;
    require_pattern_field(&error.pat, "Error", "message")?;
    require_direct_call(&svg.body, "Ok", &["svg"], "read-render-route")?;
    require_direct_call(&error.body, "Err", &["message"], "read-render-route")
}

fn require_pattern_field(pattern: &syn::Pat, variant: &str, field: &str) -> Result<(), String> {
    let syn::Pat::Struct(pattern) = pattern else {
        return Err("read-render-route:response-pattern".into());
    };
    let path = super::source::path_name(&pattern.path);
    if !path.ends_with(variant) || pattern.fields.len() != 1 {
        return Err("read-render-route:response-pattern".into());
    }
    if !matches!(&*pattern.fields[0].pat, syn::Pat::Ident(binding) if binding.ident == field) {
        return Err("read-render-route:response-binding".into());
    }
    Ok(())
}

pub(super) fn require_local_call(
    block: &syn::Block,
    index: usize,
    name: &str,
    callee: &str,
) -> Result<(), String> {
    let expression = super::source::local(block, index, name)?;
    if super::source::call_path(expression).as_deref() != Some(callee) {
        return Err(format!("read-render-route:binding:{name}"));
    }
    Ok(())
}

pub(super) fn local(block: &syn::Block, index: usize) -> Result<&syn::Expr, String> {
    let Some(syn::Stmt::Local(local)) = block.stmts.get(index) else {
        return Err("route-statement".into());
    };
    local
        .init
        .as_ref()
        .map(|init| init.expr.as_ref())
        .ok_or_else(|| "route-initializer".into())
}

pub(super) fn require_call_arg(
    expression: &syn::Expr,
    index: usize,
    name: &str,
    id: &str,
) -> Result<(), String> {
    let call = call(expression).ok_or_else(|| format!("{id}:call"))?;
    if call
        .args
        .get(index)
        .is_some_and(|argument| ident(argument, name))
    {
        Ok(())
    } else {
        Err(format!("{id}:argument:{name}"))
    }
}

pub(super) fn require_args(expression: &syn::Expr, names: &[&str], id: &str) -> Result<(), String> {
    let call = call(expression).ok_or_else(|| format!("{id}:call"))?;
    if call.args.len() != names.len() {
        return Err(format!("{id}:argument-count"));
    }
    for (argument, name) in call.args.iter().zip(names) {
        if !ident(argument, name) {
            return Err(format!("{id}:argument:{name}"));
        }
    }
    Ok(())
}

pub(super) fn require_direct_call(
    expression: &syn::Expr,
    callee: &str,
    args: &[&str],
    id: &str,
) -> Result<(), String> {
    if super::source::call_path(expression).as_deref() != Some(callee) {
        return Err(format!("{id}:callee:{callee}"));
    }
    require_args(expression, args, id)
}
