use super::super::super::source::{call, ident, local, method};
use super::{method_call, tail};
use syn::{Expr, Stmt};

#[path = "resource_snapshot_cache_guard.rs"]
mod cache;
#[path = "resource_snapshot_identity_guard.rs"]
mod identity;
#[path = "resource_snapshot_values_guard.rs"]
mod values;

const SNAPSHOT_STATEMENTS: usize = 7;
const SNAPSHOT_READ_STATEMENTS: usize = 5;
const RESOURCE_READ_INDEX: usize = 4;
const RESOURCE_REMEMBER_INDEX: usize = 5;
const SNAPSHOT_HIT_INDEX: usize = 3;
const SNAPSHOT_STABILITY_INDEX: usize = 2;

pub(super) fn inspect(file: &syn::File) -> Result<(), String> {
    identity::inspect(file)?;
    require_snapshot(file)?;
    require_snapshot_read(file)?;
    values::require_snapshot_accessors(file)?;
    cache::inspect(file)?;
    values::require_read_wrappers(file)
}

fn require_snapshot(file: &syn::File) -> Result<(), String> {
    let snapshot = method(file, "ResourceBytes", "snapshot")?;
    let block = &snapshot.block;
    if block.stmts.len() != SNAPSHOT_STATEMENTS
        || !super::super::super::syntax::call_expr_path(
            local(block, 0, "file")?,
            "File::open",
            &["path"],
        )
        || !super::super::super::syntax::call_expr_path(
            local(block, RESOURCE_READ_INDEX, "snapshot")?,
            "Self::read_snapshot",
            &["file", "before"],
        )
        || !matches!(&block.stmts[RESOURCE_REMEMBER_INDEX], Stmt::Expr(value, Some(_))
            if super::super::super::syntax::call_expr_path(value, "Self::remember", &["cache", "path", "snapshot"]))
        || !super::super::super::syntax::call_expr_path(tail(block)?, "Ok", &["snapshot"])
    {
        return Err("resource-loader:snapshot".into());
    }
    require_metadata(local(block, 1, "before")?)?;
    if !matches!(method_call(local(block, 2, "cache")?), Some(cache)
        if cache.method == "get_or_init" && ident(&cache.receiver, "CACHE")
            && cache.args.len() == 1 && ident(&cache.args[0], "Self::cache"))
    {
        return Err("resource-loader:cache".into());
    }
    require_snapshot_hit(&block.stmts[SNAPSHOT_HIT_INDEX])?;
    Ok(())
}

fn require_metadata(expression: &Expr) -> Result<(), String> {
    let Some(metadata) = call(expression) else {
        return Err("resource-loader:metadata".into());
    };
    if !ident(&metadata.func, "Identity::from_metadata")
        || metadata.args.len() != 1
        || !matches!(&metadata.args[0], Expr::Reference(value)
            if matches!(method_call(&value.expr), Some(read)
                if read.method == "metadata" && read.args.is_empty() && ident(&read.receiver, "file")))
    {
        return Err("resource-loader:metadata".into());
    }
    Ok(())
}

fn require_snapshot_hit(statement: &Stmt) -> Result<(), String> {
    let Stmt::Expr(Expr::If(branch), _) = statement else {
        return Err("resource-loader:cache".into());
    };
    if branch.else_branch.is_some()
        || branch.then_branch.stmts.len() != 1
        || !matches!(branch.cond.as_ref(), Expr::Let(value)
            if matches!(value.pat.as_ref(), syn::Pat::TupleStruct(pattern)
                if super::super::super::source::path_name(&pattern.path) == "Some"
                    && matches!(pattern.elems.first(), Some(syn::Pat::Ident(name)) if name.ident == "snapshot"))
                && super::super::super::syntax::call_expr_path(&value.expr, "Self::cached", &["cache", "path", "before"]))
        || !matches!(&branch.then_branch.stmts[0], Stmt::Expr(Expr::Return(value), Some(_))
            if value.expr.as_deref().is_some_and(|value|
                super::super::super::syntax::call_expr_path(value, "Ok", &["snapshot"])))
    {
        return Err("resource-loader:cache".into());
    }
    Ok(())
}

fn require_snapshot_read(file: &syn::File) -> Result<(), String> {
    let read = method(file, "ResourceBytes", "read_snapshot")?;
    let block = &read.block;
    if block.stmts.len() != SNAPSHOT_READ_STATEMENTS
        || !values::read_contents_call(local(block, 0, "bytes")?)
    {
        return Err("resource-loader:bytes-read".into());
    }
    require_metadata(local(block, 1, "after")?)?;
    require_stable_check(&block.stmts[SNAPSHOT_STABILITY_INDEX])?;
    values::require_snapshot_content(local(block, SNAPSHOT_STABILITY_INDEX + 1, "content")?)?;
    values::require_snapshot_result(tail(block)?)
}

fn require_stable_check(statement: &Stmt) -> Result<(), String> {
    let Stmt::Expr(Expr::If(branch), _) = statement else {
        return Err("resource-loader:changed".into());
    };
    require_stable_condition(&branch.cond)?;
    if branch.else_branch.is_some()
        || !matches!(branch.then_branch.stmts.as_slice(),
            [Stmt::Expr(Expr::Return(value), Some(_))] if matches!(value.expr.as_deref(), Some(Expr::Call(error))
                if ident(&error.func, "Err") && error.args.len() == 1
                    && matches!(call(&error.args[0]), Some(other)
                        if ident(&other.func, "std::io::Error::other") && other.args.len() == 1
                            && matches!(&other.args[0], Expr::Lit(value)
                                if matches!(&value.lit, syn::Lit::Str(text) if text.value() == "resource changed while reading")))))
    {
        return Err("resource-loader:changed".into());
    }
    Ok(())
}

fn require_stable_condition(condition: &Expr) -> Result<(), String> {
    let Expr::Binary(combined) = condition else {
        return Err("resource-loader:changed".into());
    };
    if !matches!(combined.op, syn::BinOp::Or(_))
        || !matches!(combined.left.as_ref(), Expr::Binary(value)
            if matches!(value.op, syn::BinOp::Ne(_)) && ident(&value.left, "before") && ident(&value.right, "after"))
        || !changed_length(&combined.right)
    {
        return Err("resource-loader:changed".into());
    }
    Ok(())
}

fn changed_length(expression: &Expr) -> bool {
    matches!(expression, Expr::Binary(value)
        if matches!(value.op, syn::BinOp::Ne(_)) && ident(&value.left, "before.len")
            && matches!(value.right.as_ref(), Expr::Cast(cast)
                if matches!(&*cast.ty, syn::Type::Path(path) if path.path.is_ident("u64"))
                    && matches!(method_call(&cast.expr), Some(len)
                        if len.method == "len" && len.args.is_empty() && ident(&len.receiver, "bytes"))))
}
