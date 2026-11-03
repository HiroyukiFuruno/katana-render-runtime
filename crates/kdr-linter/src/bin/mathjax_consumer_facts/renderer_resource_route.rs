use super::super::source::{call, ident, local, path_name, path_name_from_expr};

#[path = "renderer_warning_route.rs"]
mod warning_route;
pub(super) use warning_route::OutputGuard;

pub(super) fn require_resource(block: &syn::Block) -> Result<(), String> {
    require_empty_guard(block)?;
    let syn::Expr::Match(dispatch) = local(block, 2, "resource")? else {
        return Err("backend-runtime-path:resource-load".into());
    };
    if !matches!(dispatch.expr.as_ref(), syn::Expr::MethodCall(value) if value.method == "read_resource"
        && value.args.is_empty() && ident(&value.receiver, "self"))
        || dispatch.arms.len() != 2
    {
        return Err("backend-runtime-path:resource-load".into());
    }
    let [success, failure] = dispatch.arms.as_slice() else {
        return Err("backend-runtime-path:resource-load".into());
    };
    if !tuple_pattern(&success.pat, "Ok", "resource")
        || !ident(&success.body, "resource")
        || !tuple_pattern(&failure.pat, "Err", "error")
        || !resource_error_return(&failure.body)
    {
        return Err("backend-runtime-path:resource-errors".into());
    }
    Ok(())
}

fn resource_error_return(body: &syn::Expr) -> bool {
    let syn::Expr::Return(value) = body else {
        return false;
    };
    let Some(syn::Expr::Tuple(tuple)) = value.expr.as_deref() else {
        return false;
    };
    tuple.elems.len() == 2
        && ident(&tuple.elems[1], "None")
        && super::super::syntax::call_expr_path(&tuple.elems[0], "Err", &["error"])
}

pub(super) fn require_resource_methods(file: &syn::File) -> Result<(), String> {
    let read = super::super::source::method(file, "MathJaxRenderer", "read_resource")?;
    let [syn::Stmt::Expr(syn::Expr::MethodCall(map), _)] = read.block.stmts.as_slice() else {
        return Err("backend-runtime-path:resource-load".into());
    };
    if map.method != "map_err"
        || map.args.len() != 1
        || !resource_load(&map.receiver)
        || !matches!(&map.args[0], syn::Expr::Closure(value) if value.inputs.len() == 1
            && matches!(value.inputs.first(), Some(syn::Pat::Ident(name)) if name.ident == "error")
            && matches!(value.body.as_ref(), syn::Expr::MethodCall(call) if call.method == "resource_error"
                && ident(&call.receiver, "self") && call.args.len() == 1 && ident(&call.args[0], "error")))
    {
        return Err("backend-runtime-path:resource-load".into());
    }
    require_resource_error(file)
}

fn resource_load(value: &syn::Expr) -> bool {
    let Some(load) = call(value) else {
        return false;
    };
    path_name_from_expr(&load.func) == "runtime_resource_loader::load"
        && load.args.len() == 2
        && ident(&load.args[0], "self.runtime_path")
        && matches!(&load.args[1], syn::Expr::MethodCall(value) if value.method == "then_some"
            && ident(&value.receiver, "self.bundled") && value.args.len() == 1
            && ident(&value.args[0], "BundledResource::MathJax"))
}

fn require_resource_error(file: &syn::File) -> Result<(), String> {
    let error = super::super::source::method(file, "MathJaxRenderer", "resource_error")?;
    if error.block.stmts.len() != 2 {
        return Err("backend-runtime-path:resource-errors".into());
    }
    let Some(syn::Stmt::Expr(syn::Expr::If(check), _)) = error.block.stmts.first() else {
        return Err("backend-runtime-path:resource-errors".into());
    };
    if check.else_branch.is_some()
        || !matches!(check.cond.as_ref(), syn::Expr::Binary(value)
        if matches!(value.op, syn::BinOp::Eq(_)) && matches!(value.left.as_ref(), syn::Expr::MethodCall(kind)
            if kind.method == "kind" && kind.args.is_empty() && ident(&kind.receiver, "error"))
        && ident(&value.right, "std::io::ErrorKind::NotFound"))
    {
        return Err("backend-runtime-path:resource-errors".into());
    }
    require_resource_error_outputs(&error.block, &check.then_branch)
}

fn require_resource_error_outputs(block: &syn::Block, missing: &syn::Block) -> Result<(), String> {
    let [syn::Stmt::Expr(syn::Expr::Return(missing), _)] = missing.stmts.as_slice() else {
        return Err("backend-runtime-path:resource-errors".into());
    };
    let Some(syn::Stmt::Expr(other, _)) = block.stmts.last() else {
        return Err("backend-runtime-path:resource-errors".into());
    };
    if !error_format(missing.expr.as_deref(), true) || !error_format(Some(other), false) {
        return Err("backend-runtime-path:resource-errors".into());
    }
    Ok(())
}

fn error_format(value: Option<&syn::Expr>, missing: bool) -> bool {
    let Some(syn::Expr::Macro(formatted)) = value else {
        return false;
    };
    let expected = if missing {
        "\"MathJax runtime asset is not installed: {}\", self.runtime_path.display()"
    } else {
        "\"MathJax runtime asset could not be read: {}: {error}\", self.runtime_path.display()"
    };
    path_name(&formatted.mac.path) == "format"
        && expected
            .parse::<proc_macro2::TokenStream>()
            .is_ok_and(|tokens| tokens.to_string() == formatted.mac.tokens.to_string())
}

fn require_empty_guard(block: &syn::Block) -> Result<(), String> {
    let Some(syn::Stmt::Expr(syn::Expr::If(check), _)) = block.stmts.get(1) else {
        return Err("backend-runtime-path:empty-source".into());
    };
    if check.else_branch.is_some()
        || !matches!(check.cond.as_ref(), syn::Expr::MethodCall(empty)
            if empty.method == "is_empty" && empty.args.is_empty()
            && matches!(empty.receiver.as_ref(), syn::Expr::MethodCall(trim)
                if trim.method == "trim" && trim.args.is_empty() && ident(&trim.receiver, "block.source")))
    {
        return Err("backend-runtime-path:empty-source".into());
    }
    require_empty_return(&check.then_branch)
}

fn require_empty_return(block: &syn::Block) -> Result<(), String> {
    let [syn::Stmt::Expr(syn::Expr::Return(value), _)] = block.stmts.as_slice() else {
        return Err("backend-runtime-path:empty-source".into());
    };
    let Some(syn::Expr::Tuple(tuple)) = value.expr.as_deref() else {
        return Err("backend-runtime-path:empty-source".into());
    };
    if tuple.elems.len() != 2 || !ident(&tuple.elems[1], "None") {
        return Err("backend-runtime-path:empty-source".into());
    }
    let Some(err) = call(&tuple.elems[0]) else {
        return Err("backend-runtime-path:empty-source".into());
    };
    if path_name_from_expr(&err.func) != "Err"
        || err.args.len() != 1
        || !matches!(&err.args[0], syn::Expr::MethodCall(value) if value.method == "to_string"
            && value.args.is_empty() && matches!(value.receiver.as_ref(), syn::Expr::Lit(lit)
                if matches!(&lit.lit, syn::Lit::Str(text) if text.value() == "MathJax source is empty")))
    {
        return Err("backend-runtime-path:empty-source".into());
    }
    Ok(())
}

fn tuple_pattern(pattern: &syn::Pat, variant: &str, name: &str) -> bool {
    matches!(pattern, syn::Pat::TupleStruct(value) if value.path.is_ident(variant)
        && value.elems.len() == 1
        && matches!(value.elems.first(), Some(syn::Pat::Ident(binding)) if binding.ident == name))
}

pub(super) fn require_render_binding(block: &syn::Block) -> Result<(), String> {
    let Some(syn::Stmt::Local(binding)) = block.stmts.get(2) else {
        return Err("backend-runtime-path:render-dispatch".into());
    };
    if !matches!(&binding.pat, syn::Pat::Tuple(value) if value.elems.len() == 2
        && matches!(&value.elems[0], syn::Pat::Ident(name) if name.ident == "result")
        && matches!(&value.elems[1], syn::Pat::Ident(name) if name.ident == "digest"))
        || !binding.init.as_ref().is_some_and(|init|
            matches!(init.expr.as_ref(), syn::Expr::MethodCall(call) if call.method == "render_block"
                && ident(&call.receiver, "self") && call.args.len() == 1 && ident(&call.args[0], "input")))
    {
        return Err("backend-runtime-path:render-dispatch".into());
    }
    Ok(())
}
