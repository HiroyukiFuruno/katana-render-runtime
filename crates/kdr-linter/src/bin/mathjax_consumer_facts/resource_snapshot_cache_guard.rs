use super::super::super::super::source::{ident, method};
use super::super::{method_call, tail};
use syn::{Expr, Stmt};

pub(super) fn inspect(file: &syn::File) -> Result<(), String> {
    let cached = method(file, "ResourceBytes", "cached")?;
    if cached.block.stmts.len() != 2 {
        return Err("resource-loader:cache".into());
    }
    require_cache_disabled(&cached.block.stmts[0])?;
    require_cache_filter(tail(&cached.block)?)?;
    require_remember(file)
}

fn require_cache_disabled(statement: &Stmt) -> Result<(), String> {
    if !matches!(statement, Stmt::Expr(Expr::Try(value), Some(_))
        if ident(&value.expr, "identity.modified"))
    {
        return Err("resource-loader:cache".into());
    }
    Ok(())
}

fn require_cache_filter(expression: &Expr) -> Result<(), String> {
    let Some(filter) = method_call(expression) else {
        return Err("resource-loader:cache".into());
    };
    if filter.method != "filter"
        || filter.args.len() != 1
        || !matches!(method_call(&filter.receiver), Some(get)
            if get.method == "get" && get.args.len() == 1
                && ident(&get.receiver, "cache") && ident(&get.args[0], "path"))
        || !snapshot_identity_filter(&filter.args[0])
    {
        return Err("resource-loader:cache".into());
    }
    Ok(())
}

fn snapshot_identity_filter(expression: &Expr) -> bool {
    matches!(expression, Expr::Closure(value)
        if value.inputs.len() == 1 && matches!(&value.inputs[0], syn::Pat::Ident(name) if name.ident == "snapshot")
            && identity_comparison(&value.body))
}

fn identity_comparison(expression: &Expr) -> bool {
    matches!(expression, Expr::Binary(compare)
        if matches!(compare.op, syn::BinOp::Eq(_)) && ident(&compare.left, "snapshot.identity")
            && matches!(compare.right.as_ref(), Expr::Unary(deref)
                if matches!(deref.op, syn::UnOp::Deref(_)) && ident(&deref.expr, "identity")))
}

fn require_remember(file: &syn::File) -> Result<(), String> {
    let remember = method(file, "ResourceBytes", "remember")?;
    if remember.block.stmts.len() != 2 {
        return Err("resource-loader:cache".into());
    }
    let Stmt::Expr(Expr::If(branch), _) = &remember.block.stmts[1] else {
        return Err("resource-loader:cache".into());
    };
    let [
        Stmt::Expr(Expr::MethodCall(insert), Some(_)),
        Stmt::Expr(_, Some(_)),
    ] = branch.then_branch.stmts.as_slice()
    else {
        return Err("resource-loader:cache".into());
    };
    if branch.else_branch.is_some()
        || insert.method != "insert"
        || !ident(&insert.receiver, "cache")
        || insert.args.len() != 2
        || !matches!(&insert.args[0], Expr::MethodCall(path)
            if path.method == "to_path_buf" && path.args.is_empty() && ident(&path.receiver, "path"))
        || !matches!(&insert.args[1], Expr::MethodCall(snapshot)
            if snapshot.method == "clone" && snapshot.args.is_empty() && ident(&snapshot.receiver, "snapshot"))
    {
        return Err("resource-loader:cache".into());
    }
    Ok(())
}
