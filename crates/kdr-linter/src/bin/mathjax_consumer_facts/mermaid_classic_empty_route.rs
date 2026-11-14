use super::super::super::source::{Source, ident, method};
use super::super::super::syntax::call_expr_path;
use syn::Expr;

pub(super) fn inspect(source: &Source, block: &syn::Block) -> Result<(), String> {
    let Some(syn::Stmt::Expr(Expr::If(check), _)) = block.stmts.get(2) else {
        return Err("mermaid-resource-backend:empty".into());
    };
    if check.else_branch.is_some()
        || !matches!(check.cond.as_ref(), Expr::MethodCall(empty) if empty.method == "is_empty"
            && empty.args.is_empty() && matches!(empty.receiver.as_ref(), Expr::MethodCall(trim)
                if trim.method == "trim" && trim.args.is_empty() && ident(&trim.receiver, "block.source")))
    {
        return Err("mermaid-resource-backend:empty".into());
    }
    let [syn::Stmt::Expr(Expr::Return(value), _)] = check.then_branch.stmts.as_slice() else {
        return Err("mermaid-resource-backend:empty".into());
    };
    if !matches!(value.expr.as_deref(), Some(Expr::Tuple(tuple)) if tuple.elems.len() == 2
        && ident(&tuple.elems[1], "None") && matches!(&tuple.elems[0], Expr::MethodCall(call)
            if call.method == "render_empty" && ident(&call.receiver, "self") && call.args.len() == 2
                && ident(&call.args[0], "block") && ident(&call.args[1], "preset")))
    {
        return Err("mermaid-resource-backend:empty".into());
    }
    inspect_empty_helper(source)
}

fn inspect_empty_helper(source: &Source) -> Result<(), String> {
    let render = method(&source.ast, "MermaidRenderer", "render_empty")?;
    if !matches!(render.block.stmts.as_slice(), [syn::Stmt::Expr(value, _)]
        if call_expr_path(value, "MermaidRenderOps::render_mermaid_with_origin",
            &["block", "self.runtime_path", "preset", "self.bundled"]))
    {
        return Err("mermaid-resource-backend:empty".into());
    }
    Ok(())
}
