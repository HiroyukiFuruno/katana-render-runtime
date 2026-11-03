use super::super::super::source::{call, ident, local, path_name, path_name_from_expr};
use super::super::super::syntax::{require_success_render, require_warning_render};

fn digest_as_deref(value: &syn::Expr) -> bool {
    matches!(value, syn::Expr::MethodCall(call) if call.method == "as_deref"
        && call.args.is_empty() && ident(&call.receiver, "digest"))
}

pub(crate) struct OutputGuard;

impl OutputGuard {
    pub(crate) fn require_bound_success(value: &syn::Expr) -> bool {
        let syn::Expr::MethodCall(map) = value else {
            return false;
        };
        if map.method != "map" || map.args.len() != 1 || !require_success_render(&map.receiver) {
            return false;
        }
        let Some(syn::Expr::Closure(closure)) = map.args.first() else {
            return false;
        };
        if closure.inputs.len() != 1
            || !matches!(closure.inputs.first(), Some(syn::Pat::Ident(name)) if name.ident == "output")
        {
            return false;
        }
        let Some(bound) = call(&closure.body) else {
            return false;
        };
        path_name_from_expr(&bound.func) == "RenderOutputFactory::bind_resource"
            && bound.args.len() == 2
            && ident(&bound.args[0], "output")
            && digest_as_deref(&bound.args[1])
    }

    pub(crate) fn require_bound_warning(value: &syn::Expr) -> bool {
        let syn::Expr::Block(block) = value else {
            return false;
        };
        if block.block.stmts.len() != 2 {
            return false;
        }
        let Ok(key) = local(&block.block, 0, "error_key") else {
            return false;
        };
        let syn::Expr::Macro(format) = key else {
            return false;
        };
        let expected = "\"error:{}:{warning}\", digest.as_deref().unwrap_or(\"\")";
        if path_name(&format.mac.path) != "format"
            || !expected
                .parse::<proc_macro2::TokenStream>()
                .is_ok_and(|tokens| tokens.to_string() == format.mac.tokens.to_string())
        {
            return false;
        }
        let Some(syn::Stmt::Expr(value, _)) = block.block.stmts.last() else {
            return false;
        };
        require_bound_warning_result(value)
    }
}
fn require_bound_warning_result(value: &syn::Expr) -> bool {
    let Some(ok) = call(value) else {
        return false;
    };
    if path_name_from_expr(&ok.func) != "Ok" || ok.args.len() != 1 {
        return false;
    }
    let Some(bound) = call(&ok.args[0]) else {
        return false;
    };
    if path_name_from_expr(&bound.func) != "RenderOutputFactory::bind_resource"
        || bound.args.len() != 2
    {
        return false;
    }
    let Some(key) = call(&bound.args[1]) else {
        return false;
    };
    if path_name_from_expr(&key.func) != "Some"
        || key.args.len() != 1
        || !ident(&key.args[0], "error_key")
    {
        return false;
    }
    require_warning_render(&bound.args[0])
}
