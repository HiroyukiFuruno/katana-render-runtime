use super::source::{RUST_INPUTS, Source, path_name, record_fact};
use serde_json::Value;
use syn::parse::Parser;
use syn::spanned::Spanned;

#[path = "mathjax_resource_reader.rs"]
mod resource_reader;
pub(super) use resource_reader::{ResourceReader, ValidatedResourceRead};

pub(super) fn inspect_scripts(source: &Source, facts: &mut Vec<Value>) -> Result<(), String> {
    let path = RUST_INPUTS[2];
    let build = super::source::method(&source.ast, "MathJaxRuntimeScripts", "build")?;
    let expression = generated_include(source)?;
    record_fact(facts, "include-str-generated", path, expression.span());
    require_runtime_source(source)?;
    let entries = script_entries(&build.block)?;
    validate_script_order(&entries)?;
    record_fact(facts, "script-order", path, build.span());
    record_fact(facts, "global-invocation", path, build.span());
    Ok(())
}

fn generated_include(source: &Source) -> Result<&syn::ExprMacro, String> {
    let constants = source
        .ast
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Const(item) = item else {
                return None;
            };
            (item.ident == "MATHJAX_RUNTIME").then_some(item)
        })
        .collect::<Vec<_>>();
    let [constant] = constants.as_slice() else {
        return Err("include-str-generated:count".into());
    };
    let syn::Expr::Macro(expression) = constant.expr.as_ref() else {
        return Err("include-str-generated:shape".into());
    };
    if path_name(&expression.mac.path) != "include_str" {
        return Err("include-str-generated:path".into());
    }
    let literal: syn::LitStr =
        syn::parse2(expression.mac.tokens.clone()).map_err(|_| "include-str-generated:literal")?;
    if literal.value() != "../diagram_runtime/generated/mathjax-runtime.min.js" {
        return Err("include-str-generated:path".into());
    }
    Ok(expression)
}

fn require_runtime_source(source: &Source) -> Result<(), String> {
    let runtime_source =
        super::source::method(&source.ast, "MathJaxRuntimeScripts", "runtime_source")?;
    if runtime_source.block.stmts.len() != 1
        || !matches!(runtime_source.block.stmts.first(), Some(syn::Stmt::Expr(syn::Expr::Path(value), _)) if value.path.is_ident("MATHJAX_RUNTIME"))
    {
        return Err("include-str-generated:source-function".into());
    }
    Ok(())
}

fn script_entries(block: &syn::Block) -> Result<[syn::Expr; 2], String> {
    let [statement] = block.stmts.as_slice() else {
        return Err("script-order:body".into());
    };
    let syn::Stmt::Expr(syn::Expr::Macro(array), _) = statement else {
        return Err("script-order:shape".into());
    };
    if path_name(&array.mac.path) != "vec" {
        return Err("script-order:macro".into());
    }
    let entries = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        .parse2(array.mac.tokens.clone())
        .map_err(|_| "script-order:array")?;
    entries
        .into_iter()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "script-order:count".to_string())
}

fn validate_script_order([first, second]: &[syn::Expr; 2]) -> Result<(), String> {
    validate_entry(first, "mathjax-runtime.min.js", None)?;
    validate_entry(
        second,
        "render-mathjax.js",
        Some("katanaRunMathJaxRuntime({request_json});"),
    )
}

fn validate_entry(
    expression: &syn::Expr,
    expected_name: &str,
    expected_template: Option<&str>,
) -> Result<(), String> {
    let syn::Expr::Call(call) = expression else {
        return Err("script-order:entry".into());
    };
    validate_entry_name(call, expected_name)?;
    validate_entry_source(call, expected_template)
}

fn validate_entry_name(call: &syn::ExprCall, expected_name: &str) -> Result<(), String> {
    let syn::Expr::Path(callee) = call.func.as_ref() else {
        return Err("script-order:callee".into());
    };
    let expected_callee = if expected_name == "mathjax-runtime.min.js" {
        "DiagramRuntimeScript::borrowed"
    } else {
        "DiagramRuntimeScript::owned"
    };
    if path_name(&callee.path) != expected_callee || call.args.len() != 2 {
        return Err("script-order:callee".into());
    }
    let syn::Expr::Lit(name) = &call.args[0] else {
        return Err("script-order:name".into());
    };
    if !matches!(&name.lit, syn::Lit::Str(value) if value.value() == expected_name) {
        return Err("script-order:name".into());
    }
    Ok(())
}

fn validate_entry_source(
    call: &syn::ExprCall,
    expected_template: Option<&str>,
) -> Result<(), String> {
    if let Some(expected_template) = expected_template {
        let syn::Expr::Macro(formatted) = &call.args[1] else {
            return Err("global-invocation:format".into());
        };
        return validate_template(formatted, expected_template);
    }
    if !matches!(&call.args[1], syn::Expr::Path(source) if source.path.is_ident("runtime_source")) {
        return Err("script-order:source".into());
    }
    Ok(())
}

fn validate_template(formatted: &syn::ExprMacro, expected_template: &str) -> Result<(), String> {
    if path_name(&formatted.mac.path) != "format" {
        return Err("global-invocation:macro".into());
    }
    let arguments = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        .parse2(formatted.mac.tokens.clone())
        .map_err(|_| "global-invocation:arguments")?;
    if arguments.len() != 1 {
        return Err("global-invocation:arguments".into());
    }
    let syn::Expr::Lit(template) = &arguments[0] else {
        return Err("global-invocation:template".into());
    };
    let syn::Lit::Str(template) = &template.lit else {
        return Err("global-invocation:template".into());
    };
    if template.value() != expected_template {
        return Err("global-invocation:template".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "bundle_private_ast_tests.rs"]
mod private_ast_tests;
