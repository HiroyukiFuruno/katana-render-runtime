use super::{ident, method, tail, ungroup, valid_weight};
use syn::Expr;

pub(super) fn inspect(file: &syn::File) -> Result<(), String> {
    let cache = method(file, "ResourceBytes", "cache")?;
    if !super::attrs::valid(cache)
        || cache.block.stmts.len() != 1
        || !valid_cache_builder(tail(&cache.block)?)
    {
        return Err("resource-loader:cache".into());
    }
    Ok(())
}

fn exact_method<'a>(
    expression: &'a Expr,
    name: &str,
    count: usize,
) -> Option<&'a syn::ExprMethodCall> {
    let Expr::MethodCall(value) = ungroup(expression) else {
        return None;
    };
    (value.method == name && value.args.len() == count).then_some(value)
}

fn valid_cache_builder(expression: &Expr) -> bool {
    let Some(build) = exact_method(expression, "build", 0) else {
        return false;
    };
    let Expr::MethodCall(outer) = ungroup(&build.receiver) else {
        return false;
    };
    let Expr::MethodCall(inner) = ungroup(&outer.receiver) else {
        return false;
    };
    if outer.args.len() != 1 || inner.args.len() != 1 {
        return false;
    }
    let (capacity, weigher) = if outer.method == "max_capacity" && inner.method == "weigher" {
        (outer, inner)
    } else if outer.method == "weigher" && inner.method == "max_capacity" {
        (inner, outer)
    } else {
        return false;
    };
    if !ident(&capacity.args[0], "CACHE_BYTES") || !valid_weigher(&weigher.args[0]) {
        return false;
    }
    matches!(ungroup(&inner.receiver), Expr::Call(builder)
        if ident(&builder.func, "Cache::builder") && builder.args.is_empty())
}

fn argument_name(pattern: &syn::Pat) -> Option<String> {
    let pattern = match pattern {
        syn::Pat::Type(value) => value.pat.as_ref(),
        value => value,
    };
    match pattern {
        syn::Pat::Ident(value) if value.by_ref.is_none() && value.subpat.is_none() => {
            Some(value.ident.to_string())
        }
        _ => None,
    }
}

fn valid_weigher(expression: &Expr) -> bool {
    let Expr::Closure(closure) = ungroup(expression) else {
        return false;
    };
    if closure.inputs.len() != 2 {
        return false;
    }
    let (Some(path), Some(snapshot)) = (
        argument_name(&closure.inputs[0]),
        argument_name(&closure.inputs[1]),
    ) else {
        return false;
    };
    if path == snapshot {
        return false;
    }
    let body = match ungroup(&closure.body) {
        Expr::Block(block) if block.block.stmts.len() == 1 => match tail(&block.block) {
            Ok(value) => value,
            Err(_) => return false,
        },
        value => value,
    };
    valid_weight_conversion(body, &snapshot, &path)
}

fn valid_weight_conversion(expression: &Expr, snapshot: &str, path: &str) -> bool {
    let Some(fallback) = exact_method(expression, "unwrap_or", 1) else {
        return false;
    };
    if !ident(&fallback.args[0], "u32::MAX") {
        return false;
    }
    if let Some(convert) = exact_method(&fallback.receiver, "try_into", 0) {
        return valid_weight(&convert.receiver, snapshot, path);
    }
    matches!(ungroup(&fallback.receiver), Expr::Call(convert)
        if ident(&convert.func, "u32::try_from") && convert.args.len() == 1
            && valid_weight(&convert.args[0], snapshot, path))
}
