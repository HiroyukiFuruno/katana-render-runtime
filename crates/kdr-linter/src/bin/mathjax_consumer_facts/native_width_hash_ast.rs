use super::source;
use syn::parse::Parser;
use syn::{Expr, Stmt};

#[path = "native_width_hash_ast_signatures.rs"]
mod signatures;

pub(super) fn named_reference(
    argument: &syn::FnArg,
    name: &str,
    target: &str,
    mutable: bool,
) -> bool {
    signatures::named_reference(argument, name, target, mutable)
}

fn named_value(argument: &syn::FnArg, name: &str, target: &str) -> bool {
    signatures::named_value(argument, name, target)
}

fn plugin_key_signature(argument: &syn::FnArg) -> bool {
    signatures::plugin_key_signature(argument)
}

fn return_type(output: &syn::ReturnType, expected: &str) -> bool {
    signatures::return_type(output, expected)
}

#[cfg(test)]
fn type_path_is(ty: &syn::Type, expected: &str) -> bool {
    signatures::type_path_is(ty, expected)
}

pub(super) fn require_svg_id_signature(method: &syn::ImplItemFn) -> Result<(), String> {
    let parameters = method.sig.inputs.iter().collect::<Vec<_>>();
    let [source, preset] = parameters.as_slice() else {
        return Err("svg-id-hash-route:signature".into());
    };
    if !named_reference(source, "source", "str", false)
        || !named_reference(preset, "preset", "DiagramColorPreset", false)
        || !return_type(&method.sig.output, "String")
    {
        return Err("svg-id-hash-route:signature".into());
    }
    Ok(())
}

pub(super) fn require_cache_signature(method: &syn::ImplItemFn) -> Result<(), String> {
    let parameters = method.sig.inputs.iter().collect::<Vec<_>>();
    let [source, js, preset, mode, plugin_key] = parameters.as_slice() else {
        return Err("render-cache-hash-route:signature".into());
    };
    if !named_reference(source, "source", "str", false)
        || !named_reference(js, "mermaid_js", "Path", false)
        || !named_reference(preset, "preset", "DiagramColorPreset", false)
        || !named_value(mode, "mode", "DiagramRuntimeMode")
        || !plugin_key_signature(plugin_key)
        || !return_type(&method.sig.output, "PathBuf")
    {
        return Err("render-cache-hash-route:signature".into());
    }
    Ok(())
}

pub(super) fn require_format_return(
    statement: &Stmt,
    template: &str,
    value: &str,
    error: &str,
) -> Result<(), String> {
    let Some(Expr::Macro(expression)) = statement_expression(statement) else {
        return Err(error.into());
    };
    if source::path_name(&expression.mac.path) != "format" {
        return Err(error.into());
    }
    let arguments = macro_arguments(&expression.mac.tokens, error)?;
    if arguments.len() != 2
        || !string_literal(&arguments[0], template)
        || !finish_call(&arguments[1], value)
    {
        return Err(error.into());
    }
    Ok(())
}

pub(super) fn require_cache_path(statement: &Stmt) -> Result<(), String> {
    let Some(Expr::MethodCall(outer)) = statement_expression(statement) else {
        return Err("render-cache-hash-route:path".into());
    };
    require_cache_join(outer)?;
    let Expr::Macro(formatted) = &outer.args[0] else {
        return Err("render-cache-hash-route:filename".into());
    };
    require_cache_filename(formatted)
}

fn require_cache_join(outer: &syn::ExprMethodCall) -> Result<(), String> {
    let Expr::MethodCall(inner) = outer.receiver.as_ref() else {
        return Err("render-cache-hash-route:path".into());
    };
    if outer.method != "join"
        || inner.method != "join"
        || !temp_dir_call(&inner.receiver)
        || !string_argument(&inner.args, "katana_mermaid_cache")
        || outer.args.len() != 1
    {
        return Err("render-cache-hash-route:path".into());
    }
    Ok(())
}

fn require_cache_filename(formatted: &syn::ExprMacro) -> Result<(), String> {
    if source::path_name(&formatted.mac.path) != "format" {
        return Err("render-cache-hash-route:filename".into());
    }
    let arguments = macro_arguments(&formatted.mac.tokens, "render-cache-hash-route:filename")?;
    if arguments.len() != super::CACHE_FILENAME_ARGUMENT_COUNT
        || !string_literal(&arguments[0], "{:016x}.{}")
        || !finish_call(&arguments[1], "hasher.finish()")
        || !extension_call(&arguments[2])
    {
        return Err("render-cache-hash-route:filename".into());
    }
    Ok(())
}

pub(super) fn statement_expression(statement: &Stmt) -> Option<&Expr> {
    match statement {
        Stmt::Expr(expression, _) => Some(expression),
        _ => None,
    }
}

fn macro_arguments(tokens: &proc_macro2::TokenStream, error: &str) -> Result<Vec<Expr>, String> {
    syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
        .parse2(tokens.clone())
        .map(|arguments| arguments.into_iter().collect())
        .map_err(|_| error.into())
}

fn string_literal(expression: &Expr, expected: &str) -> bool {
    matches!(expression, Expr::Lit(literal) if matches!(&literal.lit, syn::Lit::Str(value) if value.value() == expected))
}

fn finish_call(expression: &Expr, expected: &str) -> bool {
    let Expr::MethodCall(call) = expression else {
        return false;
    };
    call.method == "finish"
        && call.args.is_empty()
        && source::path_name_from_expr(&call.receiver) == expected.trim_end_matches(".finish()")
}

fn extension_call(expression: &Expr) -> bool {
    let Expr::MethodCall(call) = expression else {
        return false;
    };
    call.method == "mermaid_cache_extension"
        && call.args.is_empty()
        && source::path_name_from_expr(&call.receiver) == "mode"
}

fn temp_dir_call(expression: &Expr) -> bool {
    source::call(expression).is_some_and(|call| {
        source::path_name_from_expr(&call.func) == "std::env::temp_dir" && call.args.is_empty()
    })
}

fn string_argument(
    arguments: &syn::punctuated::Punctuated<Expr, syn::Token![,]>,
    value: &str,
) -> bool {
    matches!(arguments.first(), Some(Expr::Lit(literal)) if matches!(&literal.lit, syn::Lit::Str(string) if string.value() == value))
}

#[cfg(test)]
#[path = "native_width_hash_ast_tests.rs"]
mod tests;
