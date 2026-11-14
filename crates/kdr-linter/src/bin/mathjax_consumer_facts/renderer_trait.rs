use super::consumer::local;
use super::source::{RUST_INPUTS, Source, ident, method, record_fact};
use super::syntax::{call_expr_path, return_call};
use serde_json::Value;
use std::collections::BTreeMap;
use syn::spanned::Spanned;

const RENDER_BLOCK_STMT_COUNT: usize = 6;
const RENDER_BLOCK_RETURN_INDEX: usize = 5;
const RENDER_TRAIT_STMT_COUNT: usize = 4;
const RENDER_TRAIT_RETURN_INDEX: usize = 3;
const MATHJAX_BACKEND_INPUT_INDEX: usize = 4;
const PRESET_STATEMENT_INDEX: usize = 3;
const RENDER_RESULT_STATEMENT_INDEX: usize = 4;

#[path = "renderer_loaded_call.rs"]
mod loaded_call;

#[path = "renderer_resource_route.rs"]
mod resource_route;

pub(super) fn inspect_backend_path(
    sources: &BTreeMap<&str, Source>,
    facts: &mut Vec<Value>,
) -> Result<(), String> {
    let path = RUST_INPUTS[MATHJAX_BACKEND_INPUT_INDEX];
    inspect_backend_trait_route(&sources[path].ast, path, facts)?;
    let caller = method(&sources[path].ast, "MathJaxRenderer", "render_block")?;
    require_backend_runtime_path(&caller.block)?;
    resource_route::require_resource_methods(&sources[path].ast)?;
    record_fact(facts, "backend-runtime-path", path, caller.span());
    Ok(())
}

fn require_backend_runtime_path(block: &syn::Block) -> Result<(), String> {
    if block.stmts.len() != RENDER_BLOCK_STMT_COUNT {
        return Err("backend-runtime-path:statement-count".into());
    }
    require_diagram_block(block)?;
    if !call_expr_path(
        local(block, PRESET_STATEMENT_INDEX)?,
        "DiagramColorPreset::for_render_input",
        &["input"],
    ) {
        return Err("backend-runtime-path:preset-input".into());
    }
    resource_route::require_resource(block)?;
    loaded_call::require_runtime_path_call(super::source::local(
        block,
        RENDER_RESULT_STATEMENT_INDEX,
        "result",
    )?)?;
    require_resource_digest(block)
}

fn require_resource_digest(block: &syn::Block) -> Result<(), String> {
    let Some(syn::Stmt::Expr(syn::Expr::Tuple(result), _)) =
        block.stmts.get(RENDER_BLOCK_RETURN_INDEX)
    else {
        return Err("backend-runtime-path:call".into());
    };
    if result.elems.len() != 2
        || !ident(&result.elems[0], "result")
        || !matches!(&result.elems[1], syn::Expr::MethodCall(call) if call.method == "into_digest"
            && call.args.is_empty() && ident(&call.receiver, "resource"))
    {
        return Err("backend-runtime-path:resource-digest".into());
    }
    Ok(())
}

fn inspect_backend_trait_route(
    file: &syn::File,
    path: &str,
    facts: &mut Vec<Value>,
) -> Result<(), String> {
    let render = super::source::trait_method(file, "Renderer", "MathJaxRenderer", "render")?;
    let block = &render.block;
    require_render_kind_guard(block)?;
    require_render_runtime(block)?;
    require_render_dispatch(block)?;
    record_fact(facts, "backend-render-entrypoint", path, render.span());
    Ok(())
}

fn require_render_kind_guard(block: &syn::Block) -> Result<(), String> {
    if block.stmts.len() != RENDER_TRAIT_STMT_COUNT {
        return Err("backend-runtime-path:render-route-statement-count".into());
    }
    let Some(syn::Stmt::Expr(syn::Expr::If(check), _)) = block.stmts.first() else {
        return Err("backend-runtime-path:render-kind-check".into());
    };
    let syn::Expr::Binary(condition) = check.cond.as_ref() else {
        return Err("backend-runtime-path:render-kind-check".into());
    };
    if !is_mathjax_kind_rejection(check, condition)? {
        return Err("backend-runtime-path:render-kind-check".into());
    }
    Ok(())
}

fn is_mathjax_kind_rejection(
    check: &syn::ExprIf,
    condition: &syn::ExprBinary,
) -> Result<bool, String> {
    Ok(matches!(condition.op, syn::BinOp::Ne(_))
        && super::source::path_name_from_expr(&condition.left) == "input.kind"
        && super::source::path_name_from_expr(&condition.right) == "DiagramKind::MathJax"
        && check.else_branch.is_none()
        && check.then_branch.stmts.len() == 1
        && return_call(
            Some(
                check
                    .then_branch
                    .stmts
                    .first()
                    .ok_or("backend-runtime-path:render-kind-check")?,
            ),
            "Err",
            &["RenderError::UnsupportedKind"],
        ))
}

fn require_render_runtime(block: &syn::Block) -> Result<(), String> {
    let Some(syn::Stmt::Local(runtime)) = block.stmts.get(1) else {
        return Err("backend-runtime-path:render-runtime".into());
    };
    if !matches!(&runtime.pat, syn::Pat::Ident(value) if value.ident == "runtime")
        || !runtime
            .init
            .as_ref()
            .is_some_and(|init| call_expr_path(&init.expr, "RuntimeDescriptor::mathjax", &[]))
    {
        return Err("backend-runtime-path:render-runtime".into());
    }
    Ok(())
}

fn require_render_dispatch(block: &syn::Block) -> Result<(), String> {
    resource_route::require_render_binding(block)?;
    let Some(syn::Stmt::Expr(syn::Expr::Match(dispatch), _)) =
        block.stmts.get(RENDER_TRAIT_RETURN_INDEX)
    else {
        return Err("backend-runtime-path:render-dispatch".into());
    };
    if !ident(&dispatch.expr, "result") {
        return Err("backend-runtime-path:render-dispatch".into());
    }
    let [success, warning] = dispatch.arms.as_slice() else {
        return Err("backend-runtime-path:render-dispatch".into());
    };
    if !tuple_binding(&success.pat, "Ok", "svg")
        || !resource_route::OutputGuard::require_bound_success(&success.body)
        || !tuple_binding(&warning.pat, "Err", "warning")
        || !resource_route::OutputGuard::require_bound_warning(&warning.body)
    {
        return Err("backend-runtime-path:render-arm-flow".into());
    }
    Ok(())
}

fn tuple_binding(pattern: &syn::Pat, variant: &str, binding: &str) -> bool {
    matches!(pattern, syn::Pat::TupleStruct(value) if super::source::path_name(&value.path) == variant && value.elems.len() == 1 && matches!(value.elems.first(), Some(syn::Pat::Ident(value)) if value.ident == binding))
}
fn require_diagram_block(block: &syn::Block) -> Result<(), String> {
    let expression = local(block, 0)?;
    let syn::Expr::Struct(value) = expression else {
        return Err("backend-runtime-path:block".into());
    };
    if super::source::path_name(&value.path) != "DiagramBlock" || value.fields.len() != 2 {
        return Err("backend-runtime-path:block".into());
    }
    let kind = value
        .fields
        .iter()
        .find(|field| matches!(&field.member, syn::Member::Named(name) if name == "kind"));
    let source = value
        .fields
        .iter()
        .find(|field| matches!(&field.member, syn::Member::Named(name) if name == "source"));
    if !kind.is_some_and(|field| {
        super::source::path_name_from_expr(&field.expr) == "crate::markdown::DiagramKind::MathJax"
    }) || !source.is_some_and(|field| method_chain(&field.expr, "clone", "input.source"))
    {
        return Err("backend-runtime-path:block-fields".into());
    }
    Ok(())
}
fn method_chain(expression: &syn::Expr, method_name: &str, receiver: &str) -> bool {
    matches!(expression, syn::Expr::MethodCall(call) if call.method == method_name && call.args.is_empty() && super::source::path_name_from_expr(&call.receiver) == receiver)
}

#[cfg(test)]
#[path = "renderer_trait_tests.rs"]
mod tests;
