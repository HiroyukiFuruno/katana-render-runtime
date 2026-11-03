use super::super::consumer::require_direct_call;
use super::super::source::{Source, ident, local, method};
use syn::Expr;

const ERROR: &str = "mermaid-resource-route";
const RESOURCE_READ_ARMS: usize = 4;
const EXTERNAL_READ_ARM: usize = 3;

pub(super) fn inspect(source: &Source) -> Result<(), String> {
    super::super::call_binding::ResourceLoaderGuard::require_resource_loader(&source.ast)?;
    inspect_read(source)
}

fn inspect_read(source: &Source) -> Result<(), String> {
    let read = method(&source.ast, "RuntimeResourceLoader", "read")?;
    super::scripts::parameters(&read.sig, &["path", "bundled"])?;
    if read.block.stmts.len() != 2 {
        return Err(format!("{ERROR}:read-shape"));
    }
    let Expr::Match(branch) = local(&read.block, 0, "source")? else {
        return Err(format!("{ERROR}:read-shape"));
    };
    if !ident(&branch.expr, "bundled") || branch.arms.len() != RESOURCE_READ_ARMS {
        return Err(format!("{ERROR}:read-shape"));
    }
    inspect_bundled(&branch.arms[0])?;
    inspect_external(&branch.arms[EXTERNAL_READ_ARM], super::tail(&read.block)?)
}

fn inspect_bundled(first: &syn::Arm) -> Result<(), String> {
    if !matches!(&first.pat, syn::Pat::TupleStruct(value)
        if super::super::source::path_name(&value.path) == "Some"
            && value.elems.len() == 1 && matches!(&value.elems[0], syn::Pat::Path(path)
                if super::super::source::path_name(&path.path) == "BundledResource::Mermaid"))
    {
        return Err(format!("{ERROR}:bundled"));
    }
    let Expr::Try(value) = first.body.as_ref() else {
        return Err(format!("{ERROR}:bundled"));
    };
    let Expr::MethodCall(mapped) = value.expr.as_ref() else {
        return Err(format!("{ERROR}:bundled"));
    };
    if mapped.method != "map_err"
        || mapped.args.len() != 1
        || !ident(&mapped.args[0], "std::io::Error::other")
        || !matches!(mapped.receiver.as_ref(), Expr::MethodCall(bytes)
            if bytes.method == "bytes" && bytes.args.is_empty()
                && matches!(bytes.receiver.as_ref(), Expr::Call(call)
                    if ident(&call.func, "RuntimeAsset::mermaid") && call.args.is_empty()))
    {
        return Err(format!("{ERROR}:bundled"));
    }
    Ok(())
}

fn inspect_external(arm: &syn::Arm, tail: &Expr) -> Result<(), String> {
    if !matches!(&arm.pat, syn::Pat::Ident(value)
        if value.ident == "None" && value.by_ref.is_none() && value.mutability.is_none()
            && value.subpat.is_none())
        && !matches!(&arm.pat, syn::Pat::Path(value)
            if super::super::source::path_name(&value.path) == "None")
    {
        return Err(format!("{ERROR}:external"));
    }
    let Expr::Return(returned) = arm.body.as_ref() else {
        return Err(format!("{ERROR}:external"));
    };
    let Some(Expr::MethodCall(mapped)) = returned.expr.as_deref() else {
        return Err(format!("{ERROR}:external"));
    };
    if mapped.method != "map" || mapped.args.len() != 1 || !ident(&mapped.args[0], "Cow::Owned") {
        return Err(format!("{ERROR}:external"));
    }
    require_direct_call(&mapped.receiver, "Self::read_file", &["path"], ERROR)?;
    inspect_utf8(tail)
}

fn inspect_utf8(tail: &Expr) -> Result<(), String> {
    let Expr::MethodCall(errors) = tail else {
        return Err(format!("{ERROR}:utf8"));
    };
    if errors.method != "map_err" || errors.args.len() != 1 {
        return Err(format!("{ERROR}:utf8"));
    }
    let Expr::MethodCall(borrow) = errors.receiver.as_ref() else {
        return Err(format!("{ERROR}:utf8"));
    };
    if borrow.method != "map" || borrow.args.len() != 1 || !ident(&borrow.args[0], "Cow::Borrowed")
    {
        return Err(format!("{ERROR}:utf8"));
    }
    require_direct_call(&borrow.receiver, "std::str::from_utf8", &["source"], ERROR)
}
