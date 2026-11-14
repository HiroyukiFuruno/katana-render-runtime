use super::super::consumer::require_direct_call;
use super::super::source::{Source, call, ident, local, method};
use syn::{Expr, Stmt};

#[path = "mermaid_classic_instance_route.rs"]
mod instance_route;
pub(super) use instance_route::InstanceGuard;

const ERROR: &str = "mermaid-resource-route";
const ORIGIN_STATEMENTS: usize = 4;
const LOADED_RENDER_STATEMENTS: usize = 6;
const CACHE_DIGEST_STATEMENT: usize = 3;
const SVG_RENDER_ARGUMENTS: usize = 4;
const SVG_RENDER_CACHE_ARGUMENT: usize = 3;

pub(super) fn inspect(render: &Source, loader: &Source) -> Result<(), String> {
    super::resource_loader::inspect(loader)?;
    inspect_origin(render)?;
    inspect_loaded(render)?;
    inspect_render_bundle(render)
}

fn inspect_origin(source: &Source) -> Result<(), String> {
    let render = method(
        &source.ast,
        "MermaidRenderOps",
        "render_mermaid_with_origin",
    )?;
    super::scripts::parameters(&render.sig, &["block", "mermaid_js", "preset", "bundled"])?;
    let block = &render.block;
    if block.stmts.len() != ORIGIN_STATEMENTS {
        return Err(format!("{ERROR}:origin-shape"));
    }
    let Expr::Match(load) = local(block, 2, "bundle")? else {
        return Err(format!("{ERROR}:load"));
    };
    inspect_origin_load(load)?;
    require_direct_call(
        super::tail(block)?,
        "Self::render_mermaid_with_loaded",
        &["block", "mermaid_js", "preset", "bundle"],
        ERROR,
    )
}

fn inspect_origin_load(load: &syn::ExprMatch) -> Result<(), String> {
    let Some(call) = call(&load.expr) else {
        return Err(format!("{ERROR}:load"));
    };
    if !ident(&call.func, "runtime_resource_loader::load")
        || call.args.len() != 2
        || !ident(&call.args[0], "mermaid_js")
        || !matches!(&call.args[1], Expr::MethodCall(value)
            if value.method == "then_some" && ident(&value.receiver, "bundled")
                && value.args.len() == 1 && ident(&value.args[0], "BundledResource::Mermaid"))
        || load.arms.len() != 2
        || !matches!(&load.arms[0].pat, syn::Pat::TupleStruct(pattern)
            if super::super::source::path_name(&pattern.path) == "Ok"
                && pattern.elems.len() == 1 && matches!(&pattern.elems[0], syn::Pat::Ident(name) if name.ident == "bundle"))
        || !ident(&load.arms[0].body, "bundle")
        || !matches!(&load.arms[1].pat, syn::Pat::TupleStruct(pattern)
            if super::super::source::path_name(&pattern.path) == "Err"
                && pattern.elems.len() == 1 && matches!(&pattern.elems[0], syn::Pat::Ident(name) if name.ident == "error"))
    {
        return Err(format!("{ERROR}:load"));
    }
    inspect_origin_error(&load.arms[1].body)
}

fn inspect_origin_error(expression: &Expr) -> Result<(), String> {
    let Expr::Block(block) = expression else {
        return Err(format!("{ERROR}:load"));
    };
    let [Stmt::Expr(Expr::Return(value), Some(_))] = block.block.stmts.as_slice() else {
        return Err(format!("{ERROR}:load"));
    };
    let Some(Expr::Call(error)) = value.expr.as_deref() else {
        return Err(format!("{ERROR}:load"));
    };
    if !ident(&error.func, "Self::error")
        || error.args.len() != 2
        || !ident(&error.args[0], "block")
        || !matches!(&error.args[1], Expr::Macro(value)
            if super::super::source::path_name(&value.mac.path) == "format"
                && value.mac.tokens.to_string() == "\"Failed to read Mermaid.js bundle: {error}\"")
    {
        return Err(format!("{ERROR}:load"));
    }
    Ok(())
}

fn inspect_loaded(source: &Source) -> Result<(), String> {
    let render = method(
        &source.ast,
        "MermaidRenderOps",
        "render_mermaid_with_loaded",
    )?;
    super::scripts::parameters(&render.sig, &["block", "mermaid_js", "preset", "bundle"])?;
    let block = &render.block;
    if block.stmts.len() != LOADED_RENDER_STATEMENTS {
        return Err(format!("{ERROR}:loaded-shape"));
    }
    inspect_cache_binding(block)?;
    inspect_cache_digest(&block.stmts[CACHE_DIGEST_STATEMENT])?;
    inspect_render_call(super::tail(block)?)
}

fn inspect_cache_binding(block: &syn::Block) -> Result<(), String> {
    require_direct_call(
        local(block, 1, "mode")?,
        "DiagramRuntimeMode::current",
        &[],
        ERROR,
    )?;
    require_direct_call(
        local(block, 2, "cache_file")?,
        "Self::cache_file_path",
        &["block.source", "mermaid_js", "preset", "mode"],
        ERROR,
    )?;
    if !matches!(&block.stmts[2], Stmt::Local(value)
        if matches!(&value.pat, syn::Pat::Ident(binding) if binding.mutability.is_some()))
    {
        return Err(format!("{ERROR}:cache-binding"));
    }
    Ok(())
}

fn inspect_render_call(expression: &Expr) -> Result<(), String> {
    let Expr::Call(render) = expression else {
        return Err(format!("{ERROR}:render"));
    };
    if !ident(&render.func, "Self::render_svg_with_bundle")
        || render.args.len() != SVG_RENDER_ARGUMENTS
        || !ident(&render.args[0], "block")
        || !ident(&render.args[2], "preset")
        || !ident(&render.args[SVG_RENDER_CACHE_ARGUMENT], "cache_file")
        || !matches!(&render.args[1], Expr::MethodCall(value)
            if value.method == "text" && value.args.is_empty() && ident(&value.receiver, "bundle"))
    {
        return Err(format!("{ERROR}:render"));
    }
    Ok(())
}

fn inspect_cache_digest(statement: &Stmt) -> Result<(), String> {
    let Stmt::Expr(Expr::If(branch), _) = statement else {
        return Err(format!("{ERROR}:cache-digest"));
    };
    if !matches!(branch.cond.as_ref(), Expr::Let(value)
        if matches!(value.pat.as_ref(), syn::Pat::TupleStruct(pattern)
            if super::super::source::path_name(&pattern.path) == "Some"
                && pattern.elems.len() == 1 && matches!(&pattern.elems[0], syn::Pat::Ident(name) if name.ident == "digest"))
            && matches!(value.expr.as_ref(), Expr::MethodCall(call)
            if call.method == "digest" && call.args.is_empty() && ident(&call.receiver, "bundle")))
        || branch.else_branch.is_some()
        || branch.then_branch.stmts.len() != 1
    {
        return Err(format!("{ERROR}:cache-digest"));
    }
    inspect_digest_extension(branch)
}

fn inspect_digest_extension(branch: &syn::ExprIf) -> Result<(), String> {
    let Some(Stmt::Expr(Expr::MethodCall(extension), Some(_))) = branch.then_branch.stmts.first()
    else {
        return Err(format!("{ERROR}:cache-digest"));
    };
    if extension.method != "set_extension"
        || !ident(&extension.receiver, "cache_file")
        || extension.args.len() != 1
        || !matches!(&extension.args[0], Expr::Macro(value)
            if super::super::source::path_name(&value.mac.path) == "format"
                && value.mac.tokens.to_string() == "\"{digest}.{}\" , mode . mermaid_cache_extension ()")
    {
        return Err(format!("{ERROR}:cache-digest"));
    }
    Ok(())
}

fn inspect_render_bundle(source: &Source) -> Result<(), String> {
    let render = method(&source.ast, "MermaidRenderOps", "render_svg_with_bundle")?;
    super::scripts::parameters(&render.sig, &["block", "bundle", "preset", "cache_file"])?;
    let [_, Stmt::Expr(Expr::Match(run), None)] = render.block.stmts.as_slice() else {
        return Err(format!("{ERROR}:render-shape"));
    };
    require_direct_call(
        &run.expr,
        "MermaidJsRuntimeOps::render_bundle",
        &["block.source", "bundle", "preset"],
        ERROR,
    )?;
    if run.arms.len() != 2 {
        return Err(format!("{ERROR}:render-arms"));
    }
    Ok(())
}
