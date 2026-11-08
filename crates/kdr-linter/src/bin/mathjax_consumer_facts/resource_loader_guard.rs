use super::super::source::{call, ident, local, method};
use syn::{Block, Expr, Stmt};

const LOAD_STATEMENTS: usize = 2;
const FILE_LOAD_STATEMENTS: usize = 3;
#[path = "resource_loader_accessors.rs"]
mod accessors;
#[path = "resource_snapshot_guard.rs"]
mod snapshot_guard;

pub(crate) struct ResourceLoaderGuard;

impl ResourceLoaderGuard {
    pub(crate) fn require_bytes(file: &syn::File) -> Result<(), String> {
        snapshot_guard::inspect(file)
    }

    pub(crate) fn require_resource_loader(
        file: &syn::File,
    ) -> Result<super::super::bundle::ValidatedResourceRead<'_>, String> {
        require_external_reader(file)?;
        require_load(file)?;
        accessors::require_loaded_accessors(file)?;
        super::super::bundle::ResourceReader::require_resource_read(file)
    }
}

fn method_call(expression: &Expr) -> Option<&syn::ExprMethodCall> {
    match expression {
        Expr::MethodCall(value) => Some(value),
        Expr::Try(value) => method_call(&value.expr),
        _ => None,
    }
}

fn tail(block: &Block) -> Result<&Expr, String> {
    match block.stmts.last() {
        Some(Stmt::Expr(value, None)) => Ok(value),
        _ => Err("resource-loader:tail".into()),
    }
}

fn require_external_reader(file: &syn::File) -> Result<(), String> {
    let read = method(file, "RuntimeResourceLoader", "read_file")?;
    if read.block.stmts.len() != 1
        || !super::super::syntax::call_expr_path(
            tail(&read.block)?,
            "crate::resource_bytes::ResourceBytes::read_text",
            &["path"],
        )
    {
        return Err("resource-loader:external-read".into());
    }
    Ok(())
}

fn require_load(file: &syn::File) -> Result<(), String> {
    let load = method(file, "RuntimeResourceLoader", "load")?;
    if load.block.stmts.len() != LOAD_STATEMENTS {
        return Err("resource-loader:load-body".into());
    }
    let Expr::If(branch) = local(&load.block, 0, "source")? else {
        return Err("resource-loader:source".into());
    };
    if !matches!(branch.cond.as_ref(), Expr::MethodCall(value)
        if value.method == "is_some" && value.args.is_empty() && ident(&value.receiver, "bundled"))
    {
        return Err("resource-loader:source".into());
    }
    require_load_branches(branch)?;
    require_load_result(tail(&load.block)?)
}

fn require_load_result(expression: &Expr) -> Result<(), String> {
    let Some(ok) = call(expression) else {
        return Err("resource-loader:result".into());
    };
    if !ident(&ok.func, "Ok")
        || ok.args.len() != 1
        || !matches!(&ok.args[0], Expr::Struct(value)
            if super::super::source::path_name(&value.path) == "LoadedResource"
                && value.rest.is_none() && value.fields.len() == 1
                && value.fields.iter().any(|field| matches!(&field.member,
                    syn::Member::Named(name) if name == "source") && ident(&field.expr, "source")))
    {
        return Err("resource-loader:result".into());
    }
    Ok(())
}

fn require_load_branches(branch: &syn::ExprIf) -> Result<(), String> {
    let Some((_, alternative)) = &branch.else_branch else {
        return Err("resource-loader:source".into());
    };
    let Expr::Block(file_branch) = alternative.as_ref() else {
        return Err("resource-loader:source".into());
    };
    if file_branch.block.stmts.len() != FILE_LOAD_STATEMENTS
        || !super::super::syntax::call_expr_path(
            local(&file_branch.block, 0, "snapshot")?,
            "ResourceBytes::snapshot",
            &["path"],
        )
        || !matches!(method_call(local(&file_branch.block, 1, "text")?), Some(text)
            if text.method == "text_handle" && text.args.is_empty() && ident(&text.receiver, "snapshot"))
        || !matches!(tail(&file_branch.block)?, Expr::Struct(value)
            if super::super::source::path_name(&value.path) == "ResourceSource::File"
                && value.rest.is_none() && value.fields.len() == 2
                && value.fields.iter().any(|field| matches!(&field.member, syn::Member::Named(name) if name == "snapshot") && ident(&field.expr, "snapshot"))
                && value.fields.iter().any(|field| matches!(&field.member, syn::Member::Named(name) if name == "text") && ident(&field.expr, "text")))
    {
        return Err("resource-loader:source".into());
    }
    require_static_branch(&branch.then_branch)
}

fn require_static_branch(block: &Block) -> Result<(), String> {
    let [Stmt::Expr(Expr::Match(value), None)] = block.stmts.as_slice() else {
        return Err("resource-loader:source".into());
    };
    if !super::super::syntax::call_expr_path(&value.expr, "Self::read", &["path", "bundled"])
        || value.arms.len() != 2
        || !super::super::syntax::call_expr_path(
            &value.arms[0].body,
            "ResourceSource::Static",
            &["text"],
        )
        || !matches!(&value.arms[0].pat, syn::Pat::TupleStruct(pattern)
            if super::super::source::path_name(&pattern.path) == "Cow::Borrowed"
                && matches!(pattern.elems.first(), Some(syn::Pat::Ident(name)) if name.ident == "text"))
        || !matches!(&value.arms[1].pat, syn::Pat::TupleStruct(pattern)
            if super::super::source::path_name(&pattern.path) == "Cow::Owned"
                && pattern.elems.len() == 1 && matches!(&pattern.elems[0], syn::Pat::Wild(_)))
    {
        return Err("resource-loader:source".into());
    }
    require_owned_rejection(&value.arms[1].body)
}

fn require_owned_rejection(expression: &Expr) -> Result<(), String> {
    let returned = match expression {
        Expr::Return(value) => value,
        Expr::Block(block) => match block.block.stmts.as_slice() {
            [Stmt::Expr(Expr::Return(value), Some(_))] => value,
            _ => return Err("resource-loader:source".into()),
        },
        _ => return Err("resource-loader:source".into()),
    };
    if !matches!(returned.expr.as_deref(), Some(Expr::Call(error))
        if ident(&error.func, "Err") && error.args.len() == 1
            && matches!(call(&error.args[0]), Some(other)
                if ident(&other.func, "std::io::Error::other") && other.args.len() == 1
                    && matches!(&other.args[0], Expr::Lit(value)
                        if matches!(&value.lit, syn::Lit::Str(text) if text.value() == "bundled resource is not static"))))
    {
        return Err("resource-loader:source".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "resource_loader_guard_boundary_tests.rs"]
mod boundary_tests;
