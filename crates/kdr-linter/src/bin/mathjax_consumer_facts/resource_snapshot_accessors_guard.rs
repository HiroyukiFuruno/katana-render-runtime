use super::super::super::super::super::{source, syntax};
use super::{
    Expr, call, content_pattern, ident, method, method_call, require_snapshot_digest, tail,
};

pub(in super::super) fn require_snapshot_accessors(file: &syn::File) -> Result<(), String> {
    require_digest_accessor(file)?;
    require_invalid_utf8(file)?;
    for name in ["bytes", "text", "text_handle"] {
        require_content_accessor(file, name)?;
    }
    Ok(())
}

fn require_invalid_utf8(file: &syn::File) -> Result<(), String> {
    let invalid = method(file, "ResourceSnapshot", "invalid_utf8")?;
    let Some(error) = call(tail(&invalid.block)?) else {
        return Err("resource-loader:bytes-text".into());
    };
    if invalid.block.stmts.len() != 1
        || !ident(&error.func, "std::io::Error::new")
        || error.args.len() != 2
        || !ident(&error.args[0], "std::io::ErrorKind::InvalidData")
        || !matches!(&error.args[1], Expr::Lit(value)
            if matches!(&value.lit, syn::Lit::Str(text) if text.value() == "stream did not contain valid UTF-8"))
    {
        return Err("resource-loader:bytes-text".into());
    }
    Ok(())
}

fn require_digest_accessor(file: &syn::File) -> Result<(), String> {
    let digest = method(file, "ResourceSnapshot", "digest")?;
    let Some(init) = method_call(tail(&digest.block)?) else {
        return Err("resource-loader:accessors".into());
    };
    if digest.block.stmts.len() != 1
        || init.method != "get_or_init"
        || !ident(&init.receiver, "self.digest")
        || init.args.len() != 1
    {
        return Err("resource-loader:accessors".into());
    }
    let Expr::Closure(closure) = &init.args[0] else {
        return Err("resource-loader:accessors".into());
    };
    let Expr::Block(body) = closure.body.as_ref() else {
        return Err("resource-loader:accessors".into());
    };
    if !closure.inputs.is_empty() || body.block.stmts.len() != 1 {
        return Err("resource-loader:accessors".into());
    }
    require_snapshot_digest(tail(&body.block)?)
}

fn require_content_accessor(file: &syn::File, name: &str) -> Result<(), String> {
    let value = method(file, "ResourceSnapshot", name)?;
    let Expr::Match(branch) = tail(&value.block)? else {
        return Err("resource-loader:accessors".into());
    };
    if !ident(&branch.expr, "self.content")
        || branch.arms.len() != 2
        || !content_pattern(&branch.arms[0], "ResourceContent::Text", "text")
        || !valid_text_accessor(&branch.arms[0].body, name)
    {
        return Err("resource-loader:accessors".into());
    }
    require_binary_accessor(&branch.arms[1], name)
}

fn valid_text_accessor(expression: &Expr, name: &str) -> bool {
    matches!(expression, Expr::Call(ok)
                if ident(&ok.func, "Ok") && ok.args.len() == 1
                    && ((name == "text" && matches!(method_call(&ok.args[0]), Some(text)
                        if text.method == "as_str" && text.args.is_empty() && ident(&text.receiver, "text")))
                        || (name == "text_handle"
                            && syntax::call_expr_path(&ok.args[0], "Arc::clone", &["text"]))))
        || matches!(method_call(expression), Some(bytes)
                    if name == "bytes" && bytes.method == "as_bytes" && bytes.args.is_empty()
                        && ident(&bytes.receiver, "text"))
}

fn require_binary_accessor(arm: &syn::Arm, name: &str) -> Result<(), String> {
    if name == "bytes"
        && (!content_pattern(arm, "ResourceContent::Binary", "bytes") || !ident(&arm.body, "bytes"))
    {
        return Err("resource-loader:accessors".into());
    }
    if name != "bytes"
        && (!matches!(&arm.pat, syn::Pat::TupleStruct(pattern)
                if source::path_name(&pattern.path) == "ResourceContent::Binary"
                    && pattern.elems.len() == 1 && matches!(&pattern.elems[0], syn::Pat::Wild(_)))
            || !matches!(call(&arm.body), Some(error)
                    if ident(&error.func, "Err") && error.args.len() == 1
                        && syntax::call_expr_path(&error.args[0], "Self::invalid_utf8", &[])))
    {
        return Err("resource-loader:accessors".into());
    }
    Ok(())
}
