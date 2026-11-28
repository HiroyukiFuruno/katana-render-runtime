use super::{method_call, tail};
use crate::source::{call, free_function, ident, method, path_name};
use syn::{Expr, Stmt};

const IDENTITY_FIELDS: usize = 5;
const UNIX_IDENTITY_FIELDS: usize = 4;
const IDENTITY_FILE_STATEMENTS: usize = 3;
const REGULAR_FILE_METADATA_STATEMENTS: usize = 3;

pub(super) fn inspect(file: &syn::File) -> Result<(), String> {
    let definitions = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Struct(value) if value.ident == "Identity" => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [identity] = definitions.as_slice() else {
        return Err("resource-loader:metadata".into());
    };
    if !identity.attrs.iter().any(|attr| {
        attr.path().is_ident("derive")
            && attr
                .meta
                .require_list()
                .is_ok_and(|list| list.tokens.to_string() == "PartialEq , Eq")
    }) {
        return Err("resource-loader:metadata".into());
    }
    require_identity_file(file)?;
    require_regular_file_metadata(file)
}

fn require_identity_file(file: &syn::File) -> Result<(), String> {
    let identity = method(file, "Identity", "from_file")?;
    let Some(result) = call(tail(&identity.block)?) else {
        return Err("resource-loader:metadata".into());
    };
    let Some(Expr::Struct(value)) = result.args.first() else {
        return Err("resource-loader:metadata".into());
    };
    if result.args.len() != 1
        || identity.block.stmts.len() != IDENTITY_FILE_STATEMENTS
        || !ident(&result.func, "Ok")
        || !matches!(super::local(&identity.block, 0, "metadata")?, Expr::Try(value)
            if matches!(call(&value.expr), Some(call)
                if ident(&call.func, "regular_file_metadata") && call.args.len() == 1
                    && ident(&call.args[0], "file")))
        || path_name(&value.path) != "Self"
        || value.rest.is_some()
        || value.fields.len() != IDENTITY_FIELDS
    {
        return Err("resource-loader:metadata".into());
    }
    require_identity_fields(value)?;
    require_identity_unix(value)?;
    require_identity_windows(value)
}

fn require_identity_fields(value: &syn::ExprStruct) -> Result<(), String> {
    for name in ["len", "modified", "created"] {
        let Some(field) = value
            .fields
            .iter()
            .find(|field| matches!(&field.member, syn::Member::Named(actual) if actual == name))
        else {
            return Err("resource-loader:metadata".into());
        };
        let expression = metadata_field_expression(&field.expr, name)?;
        if !matches!(method_call(expression), Some(read)
            if read.method == name && read.args.is_empty() && ident(&read.receiver, "metadata"))
        {
            return Err("resource-loader:metadata".into());
        }
    }
    Ok(())
}

fn metadata_field_expression<'a>(expression: &'a Expr, name: &str) -> Result<&'a Expr, String> {
    if name == "len" {
        return Ok(expression);
    }
    let Some(ok) = method_call(expression) else {
        return Err("resource-loader:metadata".into());
    };
    if ok.method != "ok" || !ok.args.is_empty() {
        return Err("resource-loader:metadata".into());
    }
    Ok(&ok.receiver)
}

fn require_identity_unix(value: &syn::ExprStruct) -> Result<(), String> {
    let Some(unix) = value
        .fields
        .iter()
        .find(|field| matches!(&field.member, syn::Member::Named(actual) if actual == "unix"))
    else {
        return Err("resource-loader:metadata".into());
    };
    if !matches!(&unix.expr, Expr::Tuple(tuple)
        if tuple.elems.len() == UNIX_IDENTITY_FIELDS && tuple.elems.iter().zip(["dev", "ino", "ctime", "ctime_nsec"])
            .all(|(item, name)| matches!(method_call(item), Some(value)
                if value.method == name && value.args.is_empty() && ident(&value.receiver, "metadata"))))
    {
        return Err("resource-loader:metadata".into());
    }
    Ok(())
}

fn require_identity_windows(value: &syn::ExprStruct) -> Result<(), String> {
    let Some(windows) = value
        .fields
        .iter()
        .find(|field| matches!(&field.member, syn::Member::Named(actual) if actual == "windows"))
    else {
        return Err("resource-loader:metadata".into());
    };
    if !matches!(&windows.expr, Expr::Try(handle)
        if matches!(call(&handle.expr), Some(call)
            if ident(&call.func, "same_file::Handle::from_file") && call.args.len() == 1
                && matches!(&call.args[0], Expr::Try(clone)
                    if matches!(method_call(&clone.expr), Some(call)
                        if call.method == "try_clone" && call.args.is_empty() && ident(&call.receiver, "file")))))
    {
        return Err("resource-loader:metadata".into());
    }
    Ok(())
}

fn require_regular_file_metadata(file: &syn::File) -> Result<(), String> {
    let metadata = free_function(file, "regular_file_metadata")?;
    if metadata.block.stmts.len() != REGULAR_FILE_METADATA_STATEMENTS
        || !matches!(super::local(&metadata.block, 0, "metadata")?, Expr::Try(value)
            if matches!(method_call(&value.expr), Some(call)
                if call.method == "metadata" && call.args.is_empty() && ident(&call.receiver, "file")))
        || !valid_regular_file_branch(&metadata.block.stmts[1])
        || !valid_non_regular_file_error(tail(&metadata.block)?)
    {
        return Err("resource-loader:metadata".into());
    }
    Ok(())
}

fn valid_regular_file_branch(statement: &Stmt) -> bool {
    matches!(statement, Stmt::Expr(Expr::If(branch), _)
        if branch.else_branch.is_none()
            && matches!(method_call(&branch.cond), Some(call)
                if call.method == "is_file" && call.args.is_empty() && ident(&call.receiver, "metadata"))
            && matches!(branch.then_branch.stmts.as_slice(), [Stmt::Expr(Expr::Return(value), Some(_))]
                if matches!(value.expr.as_deref(), Some(Expr::Call(call))
                    if ident(&call.func, "Ok") && call.args.len() == 1 && ident(&call.args[0], "metadata"))))
}

fn valid_non_regular_file_error(expression: &Expr) -> bool {
    matches!(call(expression), Some(error)
        if ident(&error.func, "Err") && error.args.len() == 1
            && matches!(call(&error.args[0]), Some(new)
                if ident(&new.func, "std::io::Error::new") && new.args.len() == 2
                    && ident(&new.args[0], "std::io::ErrorKind::InvalidInput")
                    && matches!(&new.args[1], Expr::Lit(value)
                        if matches!(&value.lit, syn::Lit::Str(text) if text.value() == "resource path is not a regular file"))))
}
