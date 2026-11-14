use super::super::super::super::source::{call, ident, local, method};
use super::super::{method_call, tail};
use syn::Expr;

#[path = "resource_snapshot_accessors_guard.rs"]
mod accessors;
pub(super) use accessors::require_snapshot_accessors;

const SNAPSHOT_FIELDS: usize = 3;
const READ_CONTENT_STATEMENTS: usize = 3;
pub(super) fn require_snapshot_digest(expression: &Expr) -> Result<(), String> {
    let Some(collect) = method_call(expression) else {
        return Err("resource-loader:digest".into());
    };
    let Some(map) = method_call(&collect.receiver) else {
        return Err("resource-loader:digest".into());
    };
    let Some(iter) = method_call(&map.receiver) else {
        return Err("resource-loader:digest".into());
    };
    if collect.method != "collect"
        || !collect.args.is_empty()
        || map.method != "map"
        || map.args.len() != 1
        || iter.method != "iter"
        || !iter.args.is_empty()
        || !matches!(call(&iter.receiver), Some(digest)
            if ident(&digest.func, "Sha256::digest") && digest.args.len() == 1
                && matches!(method_call(&digest.args[0]), Some(bytes)
                    if bytes.method == "bytes" && bytes.args.is_empty() && ident(&bytes.receiver, "self")))
        || !matches!(&map.args[0], Expr::Closure(value)
            if value.inputs.len() == 1 && matches!(&value.inputs[0], syn::Pat::Ident(name) if name.ident == "byte")
                && matches!(value.body.as_ref(), Expr::Macro(format)
                    if super::super::super::super::source::path_name(&format.mac.path) == "format"
                        && format.mac.tokens.to_string() == "\"{byte:02x}\""))
    {
        return Err("resource-loader:digest".into());
    }
    Ok(())
}

fn require_binary_wrapper(file: &syn::File) -> Result<(), String> {
    let read = method(file, "ResourceBytes", "read_file")?;
    if read.block.stmts.len() != 2
        || !super::super::super::super::syntax::call_expr_path(
            local(&read.block, 0, "file")?,
            "File::open",
            &["path"],
        )
        || !read_contents_call(tail(&read.block)?)
    {
        return Err("resource-loader:bytes-read".into());
    }
    require_read_contents(file)
}

pub(super) fn read_contents_call(expression: &Expr) -> bool {
    matches!(call(expression), Some(value)
        if ident(&value.func, "Self::read_contents") && value.args.len() == 1
            && matches!(&value.args[0], Expr::Reference(reference)
                if reference.mutability.is_some() && ident(&reference.expr, "file")))
}

fn require_read_contents(file: &syn::File) -> Result<(), String> {
    let read = method(file, "ResourceBytes", "read_contents")?;
    let block = &read.block;
    if block.stmts.len() != READ_CONTENT_STATEMENTS
        || !super::super::super::super::syntax::call_expr_path(
            local(block, 0, "bytes")?,
            "Vec::new",
            &[],
        )
        || !matches!(&block.stmts[1], syn::Stmt::Expr(value, Some(_))
            if matches!(method_call(value), Some(read)
                if read.method == "read_to_end" && ident(&read.receiver, "file")
                    && read.args.len() == 1 && matches!(&read.args[0], Expr::Reference(reference)
                        if reference.mutability.is_some() && ident(&reference.expr, "bytes"))))
        || !super::super::super::super::syntax::call_expr_path(tail(block)?, "Ok", &["bytes"])
    {
        return Err("resource-loader:bytes-read".into());
    }
    Ok(())
}

pub(super) fn require_snapshot_content(expression: &Expr) -> Result<(), String> {
    let Expr::Match(value) = expression else {
        return Err("resource-loader:content".into());
    };
    if !super::super::super::super::syntax::call_expr_path(
        &value.expr,
        "String::from_utf8",
        &["bytes"],
    ) || value.arms.len() != 2
        || !content_pattern(&value.arms[0], "Ok", "text")
        || !content_pattern(&value.arms[1], "Err", "error")
        || !matches!(call(&value.arms[0].body), Some(text)
            if ident(&text.func, "ResourceContent::Text") && text.args.len() == 1
                && super::super::super::super::syntax::call_expr_path(&text.args[0], "Arc::new", &["text"]))
        || !matches!(call(&value.arms[1].body), Some(binary)
            if ident(&binary.func, "ResourceContent::Binary") && binary.args.len() == 1
                && matches!(method_call(&binary.args[0]), Some(bytes)
                    if bytes.method == "into_bytes" && bytes.args.is_empty() && ident(&bytes.receiver, "error")))
    {
        return Err("resource-loader:content".into());
    }
    Ok(())
}

pub(super) fn require_snapshot_result(expression: &Expr) -> Result<(), String> {
    let Some(ok) = call(expression) else {
        return Err("resource-loader:result".into());
    };
    if !ident(&ok.func, "Ok") || ok.args.len() != 1 {
        return Err("resource-loader:result".into());
    }
    let Some(arc) = call(&ok.args[0]) else {
        return Err("resource-loader:result".into());
    };
    if !ident(&arc.func, "Arc::new")
        || arc.args.len() != 1
        || !matches!(&arc.args[0], Expr::Struct(value)
            if super::super::super::super::source::path_name(&value.path) == "ResourceSnapshot"
                && value.fields.len() == SNAPSHOT_FIELDS && value.fields.iter().any(|field|
                    matches!(&field.member, syn::Member::Named(name) if name == "content") && ident(&field.expr, "content"))
                && value.fields.iter().any(|field|
                    matches!(&field.member, syn::Member::Named(name) if name == "digest")
                        && super::super::super::super::syntax::call_expr_path(&field.expr, "OnceLock::new", &[]))
                && value.fields.iter().any(|field|
                    matches!(&field.member, syn::Member::Named(name) if name == "identity") && ident(&field.expr, "after")))
    {
        return Err("resource-loader:result".into());
    }
    Ok(())
}

fn content_pattern(arm: &syn::Arm, variant: &str, name: &str) -> bool {
    matches!(&arm.pat, syn::Pat::TupleStruct(pattern)
        if super::super::super::super::source::path_name(&pattern.path) == variant
            && pattern.elems.len() == 1
            && matches!(&pattern.elems[0], syn::Pat::Ident(value) if value.ident == name))
}

pub(super) fn require_read_wrappers(file: &syn::File) -> Result<(), String> {
    let text = method(file, "ResourceBytes", "read_text")?;
    let Some(mapped) = method_call(tail(&text.block)?) else {
        return Err("resource-loader:bytes-text".into());
    };
    if text.block.stmts.len() != 1
        || mapped.method != "map"
        || mapped.args.len() != 1
        || !ident(&mapped.args[0], "str::to_owned")
        || !matches!(method_call(&mapped.receiver), Some(value)
            if value.method == "text" && value.args.is_empty()
                && super::super::super::super::syntax::call_expr_path(&value.receiver, "Self::snapshot", &["path"]))
    {
        return Err("resource-loader:bytes-text".into());
    }
    require_binary_wrapper(file)
}
