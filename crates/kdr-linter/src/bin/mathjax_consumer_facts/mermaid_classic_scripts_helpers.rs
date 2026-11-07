use super::super::consumer::require_direct_call;
use super::source::{self, ident};
use serde_json::{Value, json};
use syn::{Expr, Stmt, parse::Parser};

pub(in super::super) fn arguments(tokens: &proc_macro2::TokenStream) -> Result<Vec<Expr>, String> {
    Ok(
        syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
            .parse2(tokens.clone())
            .map_err(|_| "mermaid-scripts:macro-arguments")?
            .into_iter()
            .collect(),
    )
}

pub(in super::super) fn literal(expression: &Expr) -> Result<String, String> {
    match expression {
        Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(text),
            ..
        }) => Ok(text.value()),
        _ => Err("mermaid-scripts:literal".into()),
    }
}

pub(in super::super) fn script_entry(
    expression: &Expr,
    name: &str,
    code: &str,
    owned: bool,
) -> Result<Value, String> {
    let Expr::Call(call) = expression else {
        return Err("mermaid-scripts:entry".into());
    };
    let constructor = if owned {
        "DiagramRuntimeScript::owned"
    } else {
        "DiagramRuntimeScript::borrowed"
    };
    if !ident(&call.func, constructor) || call.args.len() != 2 || literal(&call.args[0])? != name {
        return Err("mermaid-scripts:entry".into());
    }
    if owned {
        require_direct_call(&call.args[1], "render_script", &[code], "mermaid-scripts")?;
    } else if !ident(&call.args[1], code) {
        return Err("mermaid-scripts:entry-code".into());
    }
    Ok(
        json!({"name": literal(&call.args[0])?, "ownership": if owned { "owned" } else { "borrowed" },
        "sourceBinding": code, "producer": if owned { "render_script" } else { "identity" }}),
    )
}

pub(in super::super) fn push_entry(
    statement: Option<&Stmt>,
    name: &str,
    code: &str,
    owned: bool,
) -> Result<Value, String> {
    let Some(Stmt::Expr(Expr::MethodCall(push), Some(_))) = statement else {
        return Err("mermaid-scripts:push".into());
    };
    if push.method != "push" || !ident(&push.receiver, "scripts") || push.args.len() != 1 {
        return Err("mermaid-scripts:push".into());
    }
    script_entry(&push.args[0], name, code, owned)
}

pub(in super::super) fn macro_statement(
    statement: Option<&Stmt>,
    expected: &str,
) -> Result<(), String> {
    let Some(Stmt::Macro(actual)) = statement else {
        return Err("mermaid-classic-realm:scope-macro".into());
    };
    let Stmt::Macro(expected) =
        syn::parse_str::<Stmt>(expected).map_err(|_| "mermaid-classic-realm:scope-template")?
    else {
        return Err("mermaid-classic-realm:scope-template".into());
    };
    if source::path_name(&actual.mac.path) != source::path_name(&expected.mac.path)
        || actual.mac.tokens.to_string() != expected.mac.tokens.to_string()
        || actual.semi_token.is_none()
    {
        return Err("mermaid-classic-realm:scope-macro".into());
    }
    Ok(())
}

pub(in super::super) fn tail(block: &syn::Block) -> Result<&Expr, String> {
    match block.stmts.last() {
        Some(Stmt::Expr(tail, None)) => Ok(tail),
        _ => Err("mermaid-classic-route:tail".into()),
    }
}

pub(in super::super) fn plain_local(
    block: &syn::Block,
    index: usize,
    name: &str,
) -> Result<(), String> {
    if !matches!(block.stmts.get(index), Some(Stmt::Local(local)) if matches!(&local.pat, syn::Pat::Ident(binding) if binding.ident == name && binding.subpat.is_none() && binding.by_ref.is_none()) && local.init.as_ref().is_some_and(|init| init.diverge.is_none()))
    {
        return Err(format!("mermaid-request-route:binding:{name}"));
    }
    Ok(())
}
