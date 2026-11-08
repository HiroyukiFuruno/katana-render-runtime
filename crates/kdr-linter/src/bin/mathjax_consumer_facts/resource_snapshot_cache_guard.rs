use super::super::super::super::source::{ident, method};
use super::super::{method_call, tail};
use syn::{Expr, Stmt};

#[path = "resource_snapshot_cache_attrs_guard.rs"]
mod attrs;
#[path = "resource_snapshot_cache_constructor_guard.rs"]
mod constructor;
#[path = "resource_snapshot_cache_weight_guard.rs"]
mod weight;

use weight::valid_weight;

pub(super) fn inspect(file: &syn::File) -> Result<(), String> {
    let cached = method(file, "ResourceBytes", "cached")?;
    if !attrs::valid(cached) || cached.block.stmts.len() != 2 {
        return Err("resource-loader:cache".into());
    }
    require_cache_disabled(&cached.block.stmts[0])?;
    require_cache_filter(tail(&cached.block)?)?;
    require_remember(file)?;
    constructor::inspect(file)
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
    if !attrs::valid(remember) || remember.block.stmts.len() != 2 {
        return Err("resource-loader:cache".into());
    }
    let Stmt::Expr(Expr::If(branch), _) = &remember.block.stmts[1] else {
        return Err("resource-loader:cache".into());
    };
    let [
        Stmt::Expr(Expr::MethodCall(insert), Some(_)),
        Stmt::Expr(pending, Some(_)),
    ] = branch.then_branch.stmts.as_slice()
    else {
        return Err("resource-loader:cache".into());
    };
    if branch.else_branch.is_some()
        || !valid_remember_condition(&branch.cond)
        || !valid_weight_binding(&remember.block.stmts[0])
        || !valid_pending(pending)
        || !valid_insert(insert)
    {
        return Err("resource-loader:cache".into());
    }
    Ok(())
}

fn valid_insert(insert: &syn::ExprMethodCall) -> bool {
    insert.attrs.is_empty()
        && insert.method == "insert"
        && ident(&insert.receiver, "cache")
        && insert.args.len() == 2
        && matches!(&insert.args[0], Expr::MethodCall(path)
            if path.method == "to_path_buf" && path.args.is_empty() && ident(&path.receiver, "path"))
        && matches!(&insert.args[1], Expr::MethodCall(snapshot)
            if snapshot.method == "clone" && snapshot.args.is_empty() && ident(&snapshot.receiver, "snapshot"))
}

fn ungroup(expression: &Expr) -> &Expr {
    match expression {
        Expr::Paren(value) => ungroup(&value.expr),
        Expr::Group(value) => ungroup(&value.expr),
        _ => expression,
    }
}

fn valid_remember_condition(expression: &Expr) -> bool {
    let Expr::Binary(condition) = ungroup(expression) else {
        return false;
    };
    matches!(condition.op, syn::BinOp::And(_))
        && ((valid_modified(&condition.left) && valid_capacity(&condition.right))
            || (valid_capacity(&condition.left) && valid_modified(&condition.right)))
}

fn valid_modified(expression: &Expr) -> bool {
    let expression = ungroup(expression);
    let (expression, method) = match expression {
        Expr::Unary(value) if matches!(value.op, syn::UnOp::Not(_)) => (&*value.expr, "is_none"),
        _ => (expression, "is_some"),
    };
    matches!(method_call(ungroup(expression)), Some(call)
        if call.method == method && call.args.is_empty()
            && ident(&call.receiver, "snapshot.identity.modified"))
}

fn valid_capacity(expression: &Expr) -> bool {
    let Expr::Binary(compare) = ungroup(expression) else {
        return false;
    };
    (matches!(compare.op, syn::BinOp::Le(_))
        && ident(&compare.left, "weight")
        && valid_capacity_limit(&compare.right))
        || (matches!(compare.op, syn::BinOp::Ge(_))
            && valid_capacity_limit(&compare.left)
            && ident(&compare.right, "weight"))
}

fn valid_capacity_limit(expression: &Expr) -> bool {
    matches!(ungroup(expression), Expr::Cast(cast)
        if ident(&cast.expr, "CACHE_BYTES") && valid_usize_type(&cast.ty))
}

fn valid_usize_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Paren(value) => valid_usize_type(&value.elem),
        syn::Type::Group(value) => valid_usize_type(&value.elem),
        syn::Type::Path(path) => path.path.is_ident("usize"),
        _ => false,
    }
}

fn valid_weight_binding(statement: &Stmt) -> bool {
    let Stmt::Local(value) = statement else {
        return false;
    };
    let pattern = match &value.pat {
        syn::Pat::Type(typed) => typed.pat.as_ref(),
        pattern => pattern,
    };
    matches!(pattern, syn::Pat::Ident(name) if name.ident == "weight")
        && value
            .init
            .as_ref()
            .is_some_and(|init| valid_weight(&init.expr, "snapshot", "path"))
}

fn valid_pending(expression: &Expr) -> bool {
    let Some(call) = method_call_without_attrs(expression) else {
        return false;
    };
    call.method == "run_pending_tasks" && call.args.is_empty() && ident(&call.receiver, "cache")
}

fn method_call_without_attrs(expression: &Expr) -> Option<&syn::ExprMethodCall> {
    match expression {
        Expr::MethodCall(call) if call.attrs.is_empty() => Some(call),
        Expr::Paren(value) if value.attrs.is_empty() => method_call_without_attrs(&value.expr),
        Expr::Group(value) if value.attrs.is_empty() => method_call_without_attrs(&value.expr),
        _ => None,
    }
}
