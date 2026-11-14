use proc_macro2::Span;
use serde_json::{Value, json};

pub(super) fn path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

pub(super) fn method<'a>(
    file: &'a syn::File,
    owner: &str,
    name: &str,
) -> Result<&'a syn::ImplItemFn, String> {
    let found = matching_methods(file, owner, None, name);
    unique_method(
        found,
        format!("missing-method:{owner}::{name}"),
        format!("ambiguous-method:{owner}::{name}"),
    )
}

pub(super) fn trait_method<'a>(
    file: &'a syn::File,
    trait_name: &str,
    owner: &str,
    name: &str,
) -> Result<&'a syn::ImplItemFn, String> {
    let found = matching_methods(file, owner, Some(trait_name), name);
    unique_method(
        found,
        format!("missing-trait-method:{trait_name} for {owner}::{name}"),
        format!("ambiguous-trait-method:{trait_name} for {owner}::{name}"),
    )
}

fn matching_methods<'a>(
    file: &'a syn::File,
    owner: &str,
    trait_name: Option<&str>,
    name: &str,
) -> Vec<&'a syn::ImplItemFn> {
    file.items
        .iter()
        .filter_map(|item| impl_method(item, owner, trait_name, name))
        .collect()
}

fn impl_method<'a>(
    item: &'a syn::Item,
    owner: &str,
    trait_name: Option<&str>,
    name: &str,
) -> Option<&'a syn::ImplItemFn> {
    let syn::Item::Impl(item) = item else {
        return None;
    };
    if !impl_matches(item, owner, trait_name) {
        return None;
    }
    item.items.iter().find_map(|member| {
        let syn::ImplItem::Fn(function) = member else {
            return None;
        };
        (function.sig.ident == name).then_some(function)
    })
}

fn impl_matches(item: &syn::ItemImpl, owner: &str, trait_name: Option<&str>) -> bool {
    let syn::Type::Path(owner_path) = item.self_ty.as_ref() else {
        return false;
    };
    if path_name(&owner_path.path) != owner {
        return false;
    }
    match (trait_name, &item.trait_) {
        (None, None) => true,
        (Some(expected), Some((actual, _))) => path_name(actual) == expected,
        _ => false,
    }
}

fn unique_method(
    found: Vec<&syn::ImplItemFn>,
    missing_error: String,
    ambiguous_error: String,
) -> Result<&syn::ImplItemFn, String> {
    match found.as_slice() {
        [function] => Ok(function),
        [] => Err(missing_error),
        _ => Err(ambiguous_error),
    }
}

pub(super) fn free_function<'a>(
    file: &'a syn::File,
    name: &str,
) -> Result<&'a syn::ItemFn, String> {
    let found = file
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            (function.sig.ident == name).then_some(function)
        })
        .collect::<Vec<_>>();
    match found.as_slice() {
        [function] => Ok(function),
        [] => Err(format!("missing-function:{name}")),
        _ => Err(format!("ambiguous-function:{name}")),
    }
}

pub(super) fn local<'a>(
    block: &'a syn::Block,
    index: usize,
    name: &str,
) -> Result<&'a syn::Expr, String> {
    let Some(syn::Stmt::Local(local)) = block.stmts.get(index) else {
        return Err(format!("route-statement:{name}"));
    };
    if !matches!(&local.pat, syn::Pat::Ident(pattern) if pattern.ident == name) {
        return Err(format!("route-binding:{name}"));
    }
    local
        .init
        .as_ref()
        .map(|init| init.expr.as_ref())
        .ok_or_else(|| format!("route-initializer:{name}"))
}

pub(super) fn call(expression: &syn::Expr) -> Option<&syn::ExprCall> {
    match expression {
        syn::Expr::Call(call) => Some(call),
        syn::Expr::Try(value) => call(&value.expr),
        syn::Expr::Paren(value) => call(&value.expr),
        syn::Expr::Group(value) => call(&value.expr),
        _ => None,
    }
}

pub(super) fn call_path(expression: &syn::Expr) -> Option<String> {
    let call = call(expression)?;
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    Some(path_name(&path.path))
}

pub(super) fn ident(expression: &syn::Expr, expected: &str) -> bool {
    path_name_from_expr(expression) == expected
}

pub(super) fn path_name_from_expr(expression: &syn::Expr) -> String {
    match expression {
        syn::Expr::Path(path) => path_name(&path.path),
        syn::Expr::Field(field) => {
            let base = path_name_from_expr(&field.base);
            let syn::Member::Named(member) = &field.member else {
                return String::new();
            };
            format!("{base}.{member}")
        }
        syn::Expr::Paren(value) => path_name_from_expr(&value.expr),
        syn::Expr::Group(value) => path_name_from_expr(&value.expr),
        syn::Expr::Reference(value) => path_name_from_expr(&value.expr),
        _ => String::new(),
    }
}

pub(super) fn record_fact(facts: &mut Vec<Value>, id: &str, path: &str, span: Span) {
    let start = span.start();
    let end = span.end();
    facts.push(json!({"id": id, "path": path, "range": {
        "lineStart": start.line, "columnStart": start.column + 1,
        "lineEnd": end.line, "columnEnd": end.column + 1
    }}));
}
