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
    if !matches!(branch.cond.as_ref(), Expr::Let(value)
        if ident(&value.expr, "bundled")
            && matches!(value.pat.as_ref(), syn::Pat::TupleStruct(pattern)
                if super::super::source::path_name(&pattern.path) == "Some"
                    && pattern.elems.len() == 1
                    && matches!(&pattern.elems[0], syn::Pat::Ident(name)
                        if name.ident == "kind" && name.by_ref.is_none()
                            && name.mutability.is_none() && name.subpat.is_none())))
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
        || !matches!(local(&file_branch.block, 0, "snapshot")?, Expr::Try(value)
            if super::super::syntax::call_expr_path(&value.expr, "ResourceBytes::snapshot", &["path"]))
        || !matches!(local(&file_branch.block, 1, "text")?, Expr::Try(value)
            if matches!(value.expr.as_ref(), Expr::MethodCall(text)
                if text.method == "text_handle" && text.args.is_empty() && ident(&text.receiver, "snapshot")))
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
    let [Stmt::Expr(Expr::Call(source), None)] = block.stmts.as_slice() else {
        return Err("resource-loader:source".into());
    };
    if !ident(&source.func, "ResourceSource::Static")
        || source.args.len() != 1
        || !matches!(&source.args[0], Expr::Try(value)
            if super::super::syntax::call_expr_path(&value.expr, "Self::read_bundled", &["kind"]))
    {
        return Err("resource-loader:source".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "resource_loader_guard_boundary_tests.rs"]
mod boundary_tests;
