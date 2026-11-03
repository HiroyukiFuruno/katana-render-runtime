use super::super::source::{call, ident};

const RESOURCE_READ_ARM_COUNT: usize = 4;
const EXTERNAL_RESOURCE_ARM_INDEX: usize = 3;

pub(crate) struct ResourceReader;

impl ResourceReader {
    pub(crate) fn require_resource_read(file: &syn::File) -> Result<(), String> {
        require_resource_read(file)
    }
}

fn require_resource_read(file: &syn::File) -> Result<(), String> {
    let read = super::super::source::method(file, "RuntimeResourceLoader", "read")?;
    if read.block.stmts.len() != 2 {
        return Err("resource-loader:read-body".into());
    }
    let source = super::super::source::local(&read.block, 0, "source")?;
    let syn::Expr::Match(dispatch) = source else {
        return Err("resource-loader:read-dispatch".into());
    };
    if !ident(&dispatch.expr, "bundled") || dispatch.arms.len() != RESOURCE_READ_ARM_COUNT {
        return Err("resource-loader:read-dispatch".into());
    }
    require_mathjax_arm(&dispatch.arms[2])?;
    require_external_arm(&dispatch.arms[EXTERNAL_RESOURCE_ARM_INDEX])
}

fn require_mathjax_arm(mathjax: &syn::Arm) -> Result<(), String> {
    if !matches!(&mathjax.pat, syn::Pat::TupleStruct(value)
        if value.path.is_ident("Some") && value.elems.len() == 1
        && matches!(value.elems.first(), Some(syn::Pat::Path(path))
            if super::super::source::path_name(&path.path) == "BundledResource::MathJax"))
    {
        return Err("resource-loader:mathjax-role".into());
    }
    require_mathjax_source(&mathjax.body)
}

fn require_mathjax_source(value: &syn::Expr) -> Result<(), String> {
    let syn::Expr::Block(body) = value else {
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
    require_borrowed_source(&ok.args[0])
}

fn require_borrowed_source(value: &syn::Expr) -> Result<(), String> {
    let Some(borrowed) = call(value) else {
        return Err("resource-loader:mathjax-source".into());
    };
    if super::super::source::path_name_from_expr(&borrowed.func) != "Cow::Borrowed"
        || borrowed.args.len() != 1
    {
        return Err("resource-loader:mathjax-source".into());
    }
    let syn::Expr::Macro(include) = &borrowed.args[0] else {
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

fn require_external_arm(external: &syn::Arm) -> Result<(), String> {
    if !matches!(&external.pat, syn::Pat::Ident(value) if value.ident == "None") {
        return Err("resource-loader:external-role".into());
    }
    let syn::Expr::Return(value) = external.body.as_ref() else {
        return Err("resource-loader:external-role".into());
    };
    if !matches!(value.expr.as_deref(), Some(syn::Expr::MethodCall(map))
        if map.method == "map" && map.args.len() == 1 && ident(&map.args[0], "Cow::Owned")
        && super::super::syntax::call_expr_path(&map.receiver, "Self::read_file", &["path"]))
    {
        return Err("resource-loader:external-role".into());
    }
    Ok(())
}
