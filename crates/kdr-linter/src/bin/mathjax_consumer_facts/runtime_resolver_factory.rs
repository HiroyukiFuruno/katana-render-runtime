use super::super::super::source::{call, ident, local, method, path_name, path_name_from_expr};
use super::super::super::syntax::call_expr_path;

const DIAGRAM_KIND_COUNT: usize = 4;

pub(crate) struct FactoryGuard;

impl FactoryGuard {
    pub(crate) fn require_factory(file: &syn::File) -> Result<(), String> {
        let value = method(file, "RuntimePathResolver", "make_renderer")?;
        if value.block.stmts.len() != 2
            || !super::is_method(local(&value.block, 0, "path")?, "path", "to_path_buf")
        {
            return Err("runtime-path-resolver:loaded-renderer".into());
        }
        let Some(syn::Stmt::Expr(syn::Expr::Match(dispatch), _)) = value.block.stmts.last() else {
            return Err("runtime-path-resolver:loaded-renderer".into());
        };
        require_factory_dispatch(dispatch)
    }
}

fn require_factory_dispatch(dispatch: &syn::ExprMatch) -> Result<(), String> {
    if !ident(&dispatch.expr, "kind") || dispatch.arms.len() != DIAGRAM_KIND_COUNT {
        return Err("runtime-path-resolver:loaded-renderer".into());
    }
    let arm = &dispatch.arms[2];
    let Some(boxed) = call(&arm.body) else {
        return Err("runtime-path-resolver:loaded-renderer".into());
    };
    if !matches!(&arm.pat, syn::Pat::Path(value) if path_name(&value.path) == "DiagramKind::MathJax")
        || path_name_from_expr(&boxed.func) != "Box::new"
        || boxed.args.len() != 1
    {
        return Err("runtime-path-resolver:loaded-renderer".into());
    }
    let syn::Expr::If(choice) = &boxed.args[0] else {
        return Err("runtime-path-resolver:loaded-renderer".into());
    };
    require_factory_choice(choice)
}

fn require_factory_choice(choice: &syn::ExprIf) -> Result<(), String> {
    if !ident(&choice.cond, "bundled")
        || !matches!(choice.then_branch.stmts.as_slice(), [syn::Stmt::Expr(value, _)]
        if call_expr_path(value, "MathJaxRenderer::with_bundled_runtime_path", &["path"]))
        || !matches!(&choice.else_branch, Some((_, value)) if matches!(value.as_ref(), syn::Expr::Block(value)
            if matches!(value.block.stmts.as_slice(), [syn::Stmt::Expr(expr, _)]
                if call_expr_path(expr, "MathJaxRenderer::with_runtime_path", &["path"]))))
    {
        return Err("runtime-path-resolver:loaded-renderer".into());
    }
    Ok(())
}
