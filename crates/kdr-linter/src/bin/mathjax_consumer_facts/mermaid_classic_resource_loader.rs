use super::super::bundle::ResourceReader;
use super::super::consumer::require_direct_call;
use super::super::source::Source;
use syn::Expr;

const ERROR: &str = "mermaid-resource-route";
pub(super) fn inspect(source: &Source) -> Result<(), String> {
    let read =
        super::super::call_binding::ResourceLoaderGuard::require_resource_loader(&source.ast)?;
    inspect_read(read)
}

fn inspect_read(verified: super::super::bundle::ValidatedResourceRead<'_>) -> Result<(), String> {
    super::scripts::parameters(&verified.read.sig, &["path", "bundled"])?;
    super::scripts::parameters(&verified.bundled_read.sig, &["bundled"])?;
    inspect_bundled(verified.bundled)?;
    inspect_external(
        verified.external,
        verified.external_map,
        super::tail(&verified.bundled_read.block)?,
    )
}

fn inspect_bundled(first: &syn::Arm) -> Result<(), String> {
    if !ResourceReader::archive_arm_matches(
        first,
        "BundledResource::Mermaid",
        "RuntimeAsset::mermaid",
    ) {
        return Err(format!("{ERROR}:bundled"));
    }
    Ok(())
}

fn inspect_external(
    arm: &syn::Arm,
    mapped: &syn::ExprMethodCall,
    tail: &Expr,
) -> Result<(), String> {
    if !matches!(&arm.pat, syn::Pat::Ident(value)
        if value.ident == "None" && value.by_ref.is_none() && value.mutability.is_none()
            && value.subpat.is_none())
    {
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
    let Expr::Closure(closure) = &errors.args[0] else {
        return Err(format!("{ERROR}:utf8"));
    };
    if closure.inputs.len() != 1
        || !matches!(&closure.inputs[0], syn::Pat::Ident(value)
            if value.ident == "error" && value.by_ref.is_none() && value.mutability.is_none()
                && value.subpat.is_none())
        || !super::super::syntax::call_expr_path(
            &closure.body,
            "std::io::Error::new",
            &["std::io::ErrorKind::InvalidData", "error"],
        )
    {
        return Err(format!("{ERROR}:utf8"));
    }
    require_direct_call(&errors.receiver, "std::str::from_utf8", &["source"], ERROR)
}
