use super::super::consumer::require_direct_call;
use super::super::source::{Source, call, ident, local, method, trait_method};
use syn::Expr;

#[path = "mermaid_classic_empty_route.rs"]
mod empty_route;

const ERROR: &str = "mermaid-resource-backend";
const RENDER_BLOCK_STATEMENTS: usize = 6;
const RESOURCE_BINDING_INDEX: usize = 3;
const RESULT_BINDING_INDEX: usize = 4;
const RENDER_ENTRY_STATEMENTS: usize = 3;
const OUTPUT_FACTORY_ARGUMENTS: usize = 3;

pub(super) fn inspect(source: &Source) -> Result<(), String> {
    inspect_entry(source)?;
    let render = method(&source.ast, "MermaidRenderer", "render_block")?;
    let block = &render.block;
    if block.stmts.len() != RENDER_BLOCK_STATEMENTS {
        return Err(format!("{ERROR}:shape"));
    }
    inspect_input(local(block, 0, "block")?)?;
    empty_route::inspect(source, block)?;
    require_direct_call(
        local(block, 1, "preset")?,
        "DiagramColorPreset::for_render_input",
        &["input"],
        ERROR,
    )?;
    inspect_resource(local(block, RESOURCE_BINDING_INDEX, "resource")?)?;
    require_direct_call(
        local(block, RESULT_BINDING_INDEX, "result")?,
        "MermaidRenderOps::render_mermaid_with_loaded",
        &["block", "self.runtime_path", "preset", "resource"],
        ERROR,
    )?;
    inspect_result(super::tail(block)?)?;
    inspect_reader(source)
}

fn inspect_entry(source: &Source) -> Result<(), String> {
    let render = trait_method(&source.ast, "Renderer", "MermaidRenderer", "render")?;
    if render.block.stmts.len() != RENDER_ENTRY_STATEMENTS {
        return Err(format!("{ERROR}:entry"));
    }
    let syn::Stmt::Local(binding) = &render.block.stmts[1] else {
        return Err(format!("{ERROR}:entry"));
    };
    if !matches!(&binding.pat, syn::Pat::Tuple(value)
        if value.elems.len() == 2
            && matches!(&value.elems[0], syn::Pat::Ident(name) if name.ident == "result")
            && matches!(&value.elems[1], syn::Pat::Ident(name) if name.ident == "digest"))
        || !matches!(&binding.init, Some(value) if matches!(value.expr.as_ref(), Expr::MethodCall(call)
            if call.method == "render_block" && ident(&call.receiver, "self")
                && call.args.len() == 1 && ident(&call.args[0], "input")))
    {
        return Err(format!("{ERROR}:entry"));
    }
    inspect_entry_output(super::tail(&render.block)?)
}

fn inspect_entry_output(expression: &Expr) -> Result<(), String> {
    let Expr::MethodCall(map) = expression else {
        return Err(format!("{ERROR}:entry"));
    };
    if map.method != "map" || map.args.len() != 1 {
        return Err(format!("{ERROR}:entry"));
    }
    let Some(factory) = call(&map.receiver) else {
        return Err(format!("{ERROR}:entry"));
    };
    if !ident(&factory.func, "RenderOutputFactory::from_diagram_result")
        || factory.args.len() != OUTPUT_FACTORY_ARGUMENTS
        || !ident(&factory.args[0], "input")
        || !ident(&factory.args[1], "result")
        || !matches!(&factory.args[2], Expr::Call(value)
            if ident(&value.func, "RuntimeDescriptor::mermaid") && value.args.is_empty())
    {
        return Err(format!("{ERROR}:entry"));
    }
    inspect_entry_binding(&map.args[0])
}

fn inspect_entry_binding(expression: &Expr) -> Result<(), String> {
    let Expr::Closure(closure) = expression else {
        return Err(format!("{ERROR}:entry"));
    };
    if closure.inputs.len() != 1
        || !matches!(&closure.inputs[0], syn::Pat::Ident(name) if name.ident == "output")
    {
        return Err(format!("{ERROR}:entry"));
    }
    let Some(bind) = call(&closure.body) else {
        return Err(format!("{ERROR}:entry"));
    };
    if !ident(&bind.func, "RenderOutputFactory::bind_resource")
        || bind.args.len() != 2
        || !ident(&bind.args[0], "output")
        || !matches!(&bind.args[1], Expr::MethodCall(value)
            if value.method == "as_deref" && value.args.is_empty() && ident(&value.receiver, "digest"))
    {
        return Err(format!("{ERROR}:entry"));
    }
    Ok(())
}

fn inspect_result(expression: &Expr) -> Result<(), String> {
    let Expr::Tuple(result) = expression else {
        return Err(format!("{ERROR}:result"));
    };
    if result.elems.len() != 2
        || !ident(&result.elems[0], "result")
        || !matches!(&result.elems[1], Expr::MethodCall(value)
            if value.method == "into_digest" && value.args.is_empty() && ident(&value.receiver, "resource"))
    {
        return Err(format!("{ERROR}:result"));
    }
    Ok(())
}

fn inspect_input(expression: &Expr) -> Result<(), String> {
    let Expr::Struct(value) = expression else {
        return Err(format!("{ERROR}:input"));
    };
    if super::super::source::path_name(&value.path) != "DiagramBlock"
        || value.rest.is_some() || value.fields.len() != 2
        || !value.fields.iter().any(|field| matches!(&field.member, syn::Member::Named(name) if name == "kind")
            && ident(&field.expr, "crate::markdown::DiagramKind::Mermaid"))
        || !value.fields.iter().any(|field| matches!(&field.member, syn::Member::Named(name) if name == "source")
            && matches!(&field.expr, Expr::MethodCall(source)
                if source.method == "clone" && source.args.is_empty() && ident(&source.receiver, "input.source")))
    {
        return Err(format!("{ERROR}:input"));
    }
    Ok(())
}

fn inspect_resource(expression: &Expr) -> Result<(), String> {
    let Expr::Match(value) = expression else {
        return Err(format!("{ERROR}:read"));
    };
    if !matches!(value.expr.as_ref(), Expr::MethodCall(read)
        if read.method == "read_resource" && ident(&read.receiver, "self")
            && read.args.len() == 1 && ident(&read.args[0], "block"))
        || value.arms.len() != 2
        || !matches!(&value.arms[0].pat, syn::Pat::TupleStruct(pattern)
            if super::super::source::path_name(&pattern.path) == "Ok"
                && pattern.elems.len() == 1 && matches!(&pattern.elems[0], syn::Pat::Ident(name) if name.ident == "resource"))
        || !ident(&value.arms[0].body, "resource")
    {
        return Err(format!("{ERROR}:read"));
    }
    inspect_read_error(&value.arms[1])
}

fn inspect_read_error(arm: &syn::Arm) -> Result<(), String> {
    if !matches!(&arm.pat, syn::Pat::TupleStruct(pattern)
        if super::super::source::path_name(&pattern.path) == "Err"
            && pattern.elems.len() == 1 && matches!(&pattern.elems[0], syn::Pat::Ident(name) if name.ident == "error"))
        || !matches!(arm.body.as_ref(), Expr::Return(returned)
        if matches!(returned.expr.as_deref(), Some(Expr::Tuple(tuple))
            if tuple.elems.len() == 2 && ident(&tuple.elems[0], "error") && ident(&tuple.elems[1], "None")))
    {
        return Err(format!("{ERROR}:read"));
    }
    Ok(())
}

fn inspect_reader(source: &Source) -> Result<(), String> {
    let read = method(&source.ast, "MermaidRenderer", "read_resource")?;
    if read.block.stmts.len() != 1 {
        return Err(format!("{ERROR}:reader"));
    }
    let Expr::MethodCall(mapped) = super::tail(&read.block)? else {
        return Err(format!("{ERROR}:reader"));
    };
    let Some(load) = call(&mapped.receiver) else {
        return Err(format!("{ERROR}:reader"));
    };
    if mapped.method != "map_err"
        || mapped.args.len() != 1
        || !ident(&load.func, "runtime_resource_loader::load")
        || load.args.len() != 2
        || !ident(&load.args[0], "self.runtime_path")
        || !matches!(&load.args[1], Expr::MethodCall(value)
            if value.method == "then_some" && ident(&value.receiver, "self.bundled")
                && value.args.len() == 1 && ident(&value.args[0], "BundledResource::Mermaid"))
    {
        return Err(format!("{ERROR}:reader"));
    }
    Ok(())
}
