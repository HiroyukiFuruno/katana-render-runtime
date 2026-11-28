use super::super::source::{call, ident};

pub(crate) struct ResourceReader;

pub(crate) struct ValidatedResourceRead<'a> {
    pub(crate) read: &'a syn::ImplItemFn,
    pub(crate) bundled: &'a syn::Arm,
    pub(crate) external: &'a syn::Arm,
    pub(crate) external_map: &'a syn::ExprMethodCall,
    pub(crate) bundled_read: &'a syn::ImplItemFn,
}

impl ResourceReader {
    pub(crate) fn require_resource_read(
        file: &syn::File,
    ) -> Result<ValidatedResourceRead<'_>, String> {
        require_resource_read(file)
    }

    pub(crate) fn archive_arm_matches(arm: &syn::Arm, role: &str, asset: &str) -> bool {
        matches!(&arm.pat, syn::Pat::Path(path)
                if super::super::source::path_name(&path.path) == role)
            && matches!(arm.body.as_ref(), syn::Expr::Try(value)
                if matches!(value.expr.as_ref(), syn::Expr::MethodCall(mapped)
                    if mapped.method == "map_err"
                        && mapped.args.len() == 1
                        && ident(&mapped.args[0], "std::io::Error::other")
                        && matches!(mapped.receiver.as_ref(), syn::Expr::MethodCall(bytes)
                            if bytes.method == "bytes" && bytes.args.is_empty()
                                && super::super::syntax::call_expr_path(&bytes.receiver, asset, &[]))))
    }
}

fn require_resource_read(file: &syn::File) -> Result<ValidatedResourceRead<'_>, String> {
    let read = super::super::source::method(file, "RuntimeResourceLoader", "read")?;
    let [syn::Stmt::Expr(syn::Expr::Match(dispatch), None)] = read.block.stmts.as_slice() else {
        return Err("resource-loader:read-body".into());
    };
    if !ident(&dispatch.expr, "bundled") {
        return Err("resource-loader:read-dispatch".into());
    }
    let [bundled, external] = dispatch.arms.as_slice() else {
        return Err("resource-loader:read-dispatch".into());
    };
    require_bundled_dispatch(bundled)?;
    let external_map = require_external_arm(external)?;
    let bundled_read = super::super::source::method(file, "RuntimeResourceLoader", "read_bundled")?;
    let mermaid = require_bundled_read(bundled_read)?;
    Ok(ValidatedResourceRead {
        read,
        bundled: mermaid,
        external,
        external_map,
        bundled_read,
    })
}

fn require_bundled_dispatch(arm: &syn::Arm) -> Result<(), String> {
    if !matches!(&arm.pat, syn::Pat::TupleStruct(value)
        if value.path.is_ident("Some") && value.elems.len() == 1
        && matches!(&value.elems[0], syn::Pat::Ident(name)
            if name.ident == "kind" && name.by_ref.is_none() && name.mutability.is_none() && name.subpat.is_none()))
    {
        return Err("resource-loader:read-dispatch".into());
    }
    let syn::Expr::MethodCall(map) = arm.body.as_ref() else {
        return Err("resource-loader:read-dispatch".into());
    };
    if map.method != "map"
        || map.args.len() != 1
        || !ident(&map.args[0], "Cow::Borrowed")
        || !super::super::syntax::call_expr_path(&map.receiver, "Self::read_bundled", &["kind"])
    {
        return Err("resource-loader:read-dispatch".into());
    }
    Ok(())
}

fn require_bundled_read(read: &syn::ImplItemFn) -> Result<&syn::Arm, String> {
    if read.block.stmts.len() != 2 {
        return Err("resource-loader:read-body".into());
    }
    let source = super::super::source::local(&read.block, 0, "source")?;
    let syn::Expr::Match(dispatch) = source else {
        return Err("resource-loader:read-dispatch".into());
    };
    if !ident(&dispatch.expr, "bundled") {
        return Err("resource-loader:read-dispatch".into());
    }
    let [mermaid, drawio, mathjax] = dispatch.arms.as_slice() else {
        return Err("resource-loader:read-dispatch".into());
    };
    require_mathjax_arm(mathjax)?;
    if !ResourceReader::archive_arm_matches(
        drawio,
        "BundledResource::Drawio",
        "RuntimeAsset::drawio",
    ) {
        return Err("resource-loader:drawio-source".into());
    }
    Ok(mermaid)
}

fn require_mathjax_arm(mathjax: &syn::Arm) -> Result<(), String> {
    if !matches!(&mathjax.pat, syn::Pat::Path(path)
        if super::super::source::path_name(&path.path) == "BundledResource::MathJax")
    {
        return Err("resource-loader:mathjax-role".into());
    }
    let syn::Expr::Block(body) = mathjax.body.as_ref() else {
        return Err("resource-loader:mathjax-source".into());
    };
    let [syn::Stmt::Expr(syn::Expr::Return(value), _)] = body.block.stmts.as_slice() else {
        return Err("resource-loader:mathjax-source".into());
    };
    let Some(ok) = value.expr.as_ref().and_then(|expr| call(expr)) else {
        return Err("resource-loader:mathjax-source".into());
    };
    if super::super::source::path_name_from_expr(&ok.func) != "Ok" || ok.args.len() != 1 {
        return Err("resource-loader:mathjax-source".into());
    }
    require_mathjax_source(&ok.args[0])
}

fn require_mathjax_source(value: &syn::Expr) -> Result<(), String> {
    let syn::Expr::Macro(include) = value else {
        return Err("resource-loader:mathjax-source".into());
    };
    let path = syn::parse2::<syn::LitStr>(include.mac.tokens.clone())
        .map_err(|_| "resource-loader:mathjax-source")?;
    if super::super::source::path_name(&include.mac.path) != "include_str"
        || path.value() != "diagram_runtime/generated/mathjax-runtime.min.js"
    {
        return Err("resource-loader:mathjax-source".into());
    }
    Ok(())
}

fn require_external_arm(external: &syn::Arm) -> Result<&syn::ExprMethodCall, String> {
    if !matches!(&external.pat, syn::Pat::Ident(value) if value.ident == "None") {
        return Err("resource-loader:external-role".into());
    }
    let syn::Expr::MethodCall(map) = external.body.as_ref() else {
        return Err("resource-loader:external-role".into());
    };
    if map.method != "map"
        || map.args.len() != 1
        || !ident(&map.args[0], "Cow::Owned")
        || !super::super::syntax::call_expr_path(&map.receiver, "Self::read_file", &["path"])
    {
        return Err("resource-loader:external-role".into());
    }
    Ok(map)
}
