use super::super::source::{Source, path_name};
use super::{arguments, literal};
use proc_macro2::Span;
use serde_json::{Value, json};
use syn::{Expr, spanned::Spanned};

pub(super) fn inspect(source: &Source) -> Result<(Vec<Value>, Span), String> {
    let (mut result, mut range) = (Vec::new(), None);
    for (name, expected) in [
        (
            "MERMAID_RUNTIME",
            "../diagram_runtime/generated/mermaid-runtime.min.js",
        ),
        (
            "ZENUML_RUNTIME",
            "../diagram_runtime/generated/zenuml-runtime.min.js",
        ),
        ("MERMAID_ZENUML_RUNTIME", "/mermaid-zenuml.min.js"),
    ] {
        let constant = unique_constant(source, name)?;
        let identity = source_identity(constant, name, expected)?;
        range = Some(include_span(range, constant.span())?);
        result.push(json!({"binding": constant.ident.to_string(), "identity": identity}));
    }
    Ok((result, range.ok_or("mermaid-scripts:include-span")?))
}

fn unique_constant<'a>(source: &'a Source, name: &str) -> Result<&'a syn::ItemConst, String> {
    let mut constants = source.ast.items.iter().filter_map(|item| match item {
        syn::Item::Const(item) if item.ident == name => Some(item),
        _ => None,
    });
    let constant = constants.next().ok_or("mermaid-scripts:include-count")?;
    if constants.next().is_some() {
        return Err("mermaid-scripts:include-count".into());
    }
    Ok(constant)
}

fn source_identity(constant: &syn::ItemConst, name: &str, expected: &str) -> Result<Value, String> {
    let Expr::Macro(include) = constant.expr.as_ref() else {
        return Err("mermaid-scripts:include".into());
    };
    let args = arguments(&include.mac.tokens)?;
    if path_name(&include.mac.path) != "include_str" || args.len() != 1 {
        return Err("mermaid-scripts:include".into());
    }
    if name == "MERMAID_ZENUML_RUNTIME" {
        plugin_identity(&args[0], expected)
    } else {
        let path = literal(&args[0])?;
        if path != expected {
            return Err("mermaid-scripts:include-path".into());
        }
        Ok(json!({"kind": "include-relative", "path": path}))
    }
}

fn plugin_identity(expression: &Expr, expected: &str) -> Result<Value, String> {
    let Expr::Macro(concat) = expression else {
        return Err("mermaid-scripts:plugin".into());
    };
    let parts = arguments(&concat.mac.tokens)?;
    if path_name(&concat.mac.path) != "concat"
        || parts.len() != 2
        || literal(&parts[1])? != expected
    {
        return Err("mermaid-scripts:plugin".into());
    }
    let environment = plugin_environment(&parts[0])?;
    Ok(json!({"kind": "build-output", "environment": environment, "suffix": literal(&parts[1])?}))
}

fn plugin_environment(expression: &Expr) -> Result<String, String> {
    let Expr::Macro(env) = expression else {
        return Err("mermaid-scripts:plugin-env".into());
    };
    let env_args = arguments(&env.mac.tokens)?;
    if path_name(&env.mac.path) != "env"
        || env_args.len() != 1
        || literal(&env_args[0])? != "OUT_DIR"
    {
        return Err("mermaid-scripts:plugin-env".into());
    }
    literal(&env_args[0])
}

fn include_span(range: Option<Span>, span: Span) -> Result<Span, String> {
    match range {
        None => Ok(span),
        Some(previous) => previous
            .join(span)
            .ok_or_else(|| "mermaid-scripts:include-span".into()),
    }
}
