use super::super::source::{call, ident, local, method, path_name, path_name_from_expr};
use super::super::syntax::call_expr_path;

#[path = "runtime_resolver_factory.rs"]
mod factory;

const RESOLVER_STATEMENT_COUNT: usize = 6;
const RESOLVED_PATH_STATEMENT_INDEX: usize = 3;
const RENDERER_STATEMENT_INDEX: usize = 4;
const DIAGRAM_KIND_COUNT: usize = 4;

pub(super) fn require_loaded_resolver(file: &syn::File, block: &syn::Block) -> Result<(), String> {
    if block.stmts.len() != RESOLVER_STATEMENT_COUNT {
        return Err("runtime-path-resolver:loaded-statement-count".into());
    }
    if !call_expr_path(
        local(block, 0, "env_name")?,
        "Self::runtime_env_name",
        &["kind"],
    ) {
        return Err("runtime-path-resolver:loaded-env".into());
    }
    require_env_value(block)?;
    require_bundled(block)?;
    require_main_path(block)?;
    require_main_renderer(block)?;
    require_env_name(file)?;
    require_selected_path(file)?;
    factory::FactoryGuard::require_factory(file)?;
    require_loaded_result(block)
}

fn require_loaded_result(block: &syn::Block) -> Result<(), String> {
    let Some(syn::Stmt::Expr(value, _)) = block.stmts.last() else {
        return Err("runtime-path-resolver:loaded-result".into());
    };
    let Some(ok) = call(value) else {
        return Err("runtime-path-resolver:loaded-result".into());
    };
    if path_name_from_expr(&ok.func) != "Ok"
        || ok.args.len() != 1
        || !matches!(&ok.args[0], syn::Expr::Tuple(tuple) if tuple.elems.len() == 2
            && ident(&tuple.elems[0], "path") && ident(&tuple.elems[1], "renderer"))
    {
        return Err("runtime-path-resolver:loaded-result".into());
    }
    Ok(())
}

fn is_method(value: &syn::Expr, receiver: &str, name: &str) -> bool {
    matches!(value, syn::Expr::MethodCall(call) if call.method == name
        && call.args.is_empty() && ident(&call.receiver, receiver))
}

fn require_env_value(block: &syn::Block) -> Result<(), String> {
    let syn::Expr::If(env) = local(block, 1, "env_value")? else {
        return Err("runtime-path-resolver:loaded-env".into());
    };
    if !is_method(&env.cond, "runtime_path", "is_none")
        || env.then_branch.stmts.len() != 1
        || !matches!(env.then_branch.stmts.first(), Some(syn::Stmt::Expr(syn::Expr::MethodCall(value), _))
            if value.method == "and_then" && ident(&value.receiver, "env_name")
            && value.args.len() == 1 && ident(&value.args[0], "std::env::var_os"))
        || !matches!(&env.else_branch, Some((_, value)) if matches!(value.as_ref(), syn::Expr::Block(value)
            if matches!(value.block.stmts.as_slice(), [syn::Stmt::Expr(expr, _)] if ident(expr, "None"))))
    {
        return Err("runtime-path-resolver:loaded-env".into());
    }
    Ok(())
}

fn require_bundled(block: &syn::Block) -> Result<(), String> {
    let syn::Expr::Binary(value) = local(block, 2, "bundled")? else {
        return Err("runtime-path-resolver:loaded-origin".into());
    };
    if !matches!(value.op, syn::BinOp::And(_))
        || !is_method(&value.right, "env_value", "is_none")
        || !matches!(value.left.as_ref(), syn::Expr::Binary(left) if matches!(left.op, syn::BinOp::And(_))
            && is_method(&left.left, "runtime_path", "is_none") && is_method(&left.right, "env_name", "is_some"))
    {
        return Err("runtime-path-resolver:loaded-origin".into());
    }
    Ok(())
}

fn require_main_path(block: &syn::Block) -> Result<(), String> {
    let syn::Expr::Match(value) = local(block, RESOLVED_PATH_STATEMENT_INDEX, "path")? else {
        return Err("runtime-path-resolver:loaded-path".into());
    };
    if !ident(&value.expr, "runtime_path") {
        return Err("runtime-path-resolver:loaded-path".into());
    }
    let [provided, selected] = value.arms.as_slice() else {
        return Err("runtime-path-resolver:loaded-path".into());
    };
    if !matches!(&provided.pat, syn::Pat::TupleStruct(pat) if pat.path.is_ident("Some") && pat.elems.len() == 1
        && matches!(pat.elems.first(), Some(syn::Pat::Ident(name)) if name.ident == "path"))
        || !ident(&provided.body, "path")
        || !matches!(&selected.pat, syn::Pat::Ident(name) if name.ident == "None")
        || !matches!(selected.body.as_ref(), syn::Expr::Try(value)
            if call_expr_path(&value.expr, "Self::resolve_selected_path", &["kind", "env_value", "plantuml_cache_dir"]))
    {
        return Err("runtime-path-resolver:loaded-path".into());
    }
    Ok(())
}

fn require_main_renderer(block: &syn::Block) -> Result<(), String> {
    if !call_expr_path(
        local(block, RENDERER_STATEMENT_INDEX, "renderer")?,
        "Self::make_renderer",
        &["kind", "path", "bundled"],
    ) {
        return Err("runtime-path-resolver:loaded-renderer".into());
    }
    Ok(())
}

fn require_env_name(file: &syn::File) -> Result<(), String> {
    let value = method(file, "RuntimePathResolver", "runtime_env_name")?;
    let [syn::Stmt::Expr(syn::Expr::Match(dispatch), _)] = value.block.stmts.as_slice() else {
        return Err("runtime-path-resolver:loaded-env".into());
    };
    if !ident(&dispatch.expr, "kind") || dispatch.arms.len() != DIAGRAM_KIND_COUNT {
        return Err("runtime-path-resolver:loaded-env".into());
    }
    let arm = &dispatch.arms[2];
    if !matches!(&arm.pat, syn::Pat::Path(value) if path_name(&value.path) == "DiagramKind::MathJax")
        || !matches!(call(&arm.body), Some(value) if path_name_from_expr(&value.func) == "Some"
            && value.args.len() == 1 && matches!(&value.args[0], syn::Expr::Lit(lit)
                if matches!(&lit.lit, syn::Lit::Str(text) if text.value() == "MATHJAX_JS")))
    {
        return Err("runtime-path-resolver:loaded-env".into());
    }
    Ok(())
}

fn require_selected_path(file: &syn::File) -> Result<(), String> {
    let value = method(file, "RuntimePathResolver", "resolve_selected_path")?;
    let [syn::Stmt::Expr(syn::Expr::MethodCall(map), _)] = value.block.stmts.as_slice() else {
        return Err("runtime-path-resolver:loaded-path".into());
    };
    if map.method != "map_err"
        || map.args.len() != 1
        || !ident(&map.args[0], "RenderError::RuntimeResolution")
    {
        return Err("runtime-path-resolver:loaded-path".into());
    }
    let syn::Expr::Match(dispatch) = map.receiver.as_ref() else {
        return Err("runtime-path-resolver:loaded-path".into());
    };
    require_selected_dispatch(dispatch)
}

fn require_selected_dispatch(dispatch: &syn::ExprMatch) -> Result<(), String> {
    if !ident(&dispatch.expr, "kind") || dispatch.arms.len() != DIAGRAM_KIND_COUNT {
        return Err("runtime-path-resolver:loaded-path".into());
    }
    let arm = &dispatch.arms[2];
    let Some(resolve) = call(&arm.body) else {
        return Err("runtime-path-resolver:loaded-path".into());
    };
    if !matches!(&arm.pat, syn::Pat::Path(value) if path_name(&value.path) == "DiagramKind::MathJax")
        || path_name_from_expr(&resolve.func) != "MathJaxRendererOps::resolve_mathjax_js_with_env"
        || resolve.args.len() != 2
        || !ident(&resolve.args[0], "env_value")
        || !call_expr_path(
            &resolve.args[1],
            "MathJaxRendererOps::default_install_path",
            &[],
        )
    {
        return Err("runtime-path-resolver:loaded-path".into());
    }
    Ok(())
}
