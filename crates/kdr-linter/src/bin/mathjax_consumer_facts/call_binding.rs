use super::consumer::require_direct_call;
use super::source::{call, ident};
use syn::parse::Parser;

#[path = "resource_loader_guard.rs"]
mod resource_loader_guard;
pub(super) use resource_loader_guard::ResourceLoaderGuard;

pub(super) fn require_request_json(expression: &syn::Expr) -> Result<(), String> {
    let syn::Expr::MethodCall(to_string) = expression else {
        return Err("read-render-route:request-json".into());
    };
    if to_string.method != "to_string" || !to_string.args.is_empty() {
        return Err("read-render-route:request-json".into());
    }
    let syn::Expr::MethodCall(to_json) = to_string.receiver.as_ref() else {
        return Err("read-render-route:request-json".into());
    };
    if to_json.method != "to_json_value"
        || !to_json.args.is_empty()
        || !ident(&to_json.receiver, "request")
    {
        return Err("read-render-route:request-json".into());
    }
    Ok(())
}

pub(super) fn require_runtime_source_reader(file: &syn::File) -> Result<(), String> {
    let read = source_reader_match(file)?;
    validate_source_reader_arms(read)
}

fn source_reader_match(file: &syn::File) -> Result<&syn::ExprMatch, String> {
    let reader = super::source::free_function(file, "runtime_source_from")?;
    if reader.block.stmts.len() != 1 {
        return Err("read-render-route:source-reader-body".into());
    }
    let syn::Stmt::Expr(syn::Expr::Match(read), _) = &reader.block.stmts[0] else {
        return Err("read-render-route:source-reader-match".into());
    };
    let Some(read_call) = call(&read.expr) else {
        return Err("read-render-route:source-reader-call".into());
    };
    if super::source::path_name_from_expr(&read_call.func) != "runtime_resource_loader::read_file"
        || read_call.args.len() != 1
        || !ident(&read_call.args[0], "mathjax_js")
    {
        return Err("read-render-route:source-reader-call".into());
    }
    Ok(read)
}

fn validate_source_reader_arms(read: &syn::ExprMatch) -> Result<(), String> {
    let [success, missing, other] = read.arms.as_slice() else {
        return Err("read-render-route:source-reader-call".into());
    };
    let (success_pattern, success_guard) = guarded_pattern(&success.pat);
    let (missing_pattern, missing_guard) = guarded_pattern(&missing.pat);
    let (other_pattern, other_guard) = guarded_pattern(&other.pat);
    require_success_arm(success_pattern, success_guard, &success.body)?;
    require_missing_arm(missing_pattern, missing_guard, &missing.body)?;
    require_other_arm(other_pattern, other_guard, &other.body)?;
    Ok(())
}

fn require_success_arm(
    pattern: &syn::Pat,
    guard: Option<&syn::Expr>,
    body: &syn::Expr,
) -> Result<(), String> {
    if !tuple_pattern(pattern, "Ok", "source") || guard.is_some() {
        return Err("read-render-route:source-reader-success-pattern".into());
    }
    require_direct_call(body, "Ok", &["source"], "read-render-route")
}

fn require_missing_arm(
    pattern: &syn::Pat,
    guard: Option<&syn::Expr>,
    body: &syn::Expr,
) -> Result<(), String> {
    if !tuple_pattern(pattern, "Err", "error") || !guard.is_some_and(is_not_found_guard) {
        return Err("read-render-route:source-reader-not-found-pattern".into());
    }
    if !require_formatted_error(
        body,
        "MathJax runtime asset is not installed: {}",
        &["mathjax_js.display()"],
    ) {
        return Err("read-render-route:source-reader-not-found-error".into());
    }
    Ok(())
}

fn require_other_arm(
    pattern: &syn::Pat,
    guard: Option<&syn::Expr>,
    body: &syn::Expr,
) -> Result<(), String> {
    if !tuple_pattern(pattern, "Err", "error") || guard.is_some() {
        return Err("read-render-route:source-reader-other-pattern".into());
    }
    if !require_formatted_error(
        body,
        "MathJax runtime asset could not be read: {}: {error}",
        &["mathjax_js.display()"],
    ) {
        return Err("read-render-route:source-reader-other-error".into());
    }
    Ok(())
}
fn guarded_pattern(pattern: &syn::Pat) -> (&syn::Pat, Option<&syn::Expr>) {
    match pattern {
        syn::Pat::Guard(value) => (&value.pat, Some(&value.guard)),
        pattern => (pattern, None),
    }
}
fn tuple_pattern(pattern: &syn::Pat, variant: &str, binding: &str) -> bool {
    matches!(pattern, syn::Pat::TupleStruct(value) if super::source::path_name(&value.path) == variant && value.elems.len() == 1 && matches!(value.elems.first(), Some(syn::Pat::Ident(value)) if value.ident == binding))
}
fn is_not_found_guard(expression: &syn::Expr) -> bool {
    let syn::Expr::Binary(binary) = expression else {
        return false;
    };
    matches!(binary.op, syn::BinOp::Eq(_))
        && matches!(&*binary.left, syn::Expr::MethodCall(kind) if kind.method == "kind" && kind.args.is_empty() && ident(&kind.receiver, "error"))
        && ident(&binary.right, "std::io::ErrorKind::NotFound")
}
fn require_formatted_error(expression: &syn::Expr, template: &str, args: &[&str]) -> bool {
    let Some(values) = formatted_error_values(expression) else {
        return false;
    };
    matches_formatted_error(&values, template, args)
}

fn formatted_error_values(
    expression: &syn::Expr,
) -> Option<syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>> {
    let error = call(expression)?;
    if super::source::path_name_from_expr(&error.func) != "Err" || error.args.len() != 1 {
        return None;
    }
    let syn::Expr::Macro(formatted) = &error.args[0] else {
        return None;
    };
    if super::source::path_name(&formatted.mac.path) != "format" {
        return None;
    }
    match syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        .parse2(formatted.mac.tokens.clone())
    {
        Ok(values) => Some(values),
        Err(_) => {
            eprintln!("format tokens: {:?}", formatted.mac.tokens);
            None
        }
    }
}

fn matches_formatted_error(
    values: &syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>,
    template: &str,
    args: &[&str],
) -> bool {
    if values.len() != args.len() + 1
        || !matches!(values.first(), Some(syn::Expr::Lit(value)) if matches!(&value.lit, syn::Lit::Str(text) if text.value() == template))
    {
        return false;
    }
    values.iter().skip(1).zip(args).all(|(actual, expected)| {
        if *expected == "mathjax_js.display()" {
            matches!(actual, syn::Expr::MethodCall(display) if display.method == "display" && display.args.is_empty() && ident(&display.receiver, "mathjax_js"))
        } else {
            ident(actual, expected)
        }
    })
}
