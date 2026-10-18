use super::source::{Source, call, path_name_from_expr};
use super::syntax::local_or_tail;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn inspect_overrides(
    sources: &BTreeMap<&str, Source>,
    facts: &mut Vec<Value>,
) -> Result<(), String> {
    super::default_materialize::inspect_default_override(sources, facts)?;
    super::runtime_dispatch::inspect_runtime_resolver(sources, facts)?;
    super::renderer_trait::inspect_backend_path(sources, facts)
}

pub(super) fn require_default_install_path(block: &syn::Block) -> Result<(), String> {
    let expression = default_install_path_value(block)?;
    let root = require_install_path_joins(expression)?;
    require_temp_dir_root(root)
}

fn default_install_path_value(block: &syn::Block) -> Result<&syn::Expr, String> {
    if block.stmts.len() != 1 {
        return Err("default-materialize:install-path-statement-count".into());
    }
    let Some(some) = call(local_or_tail(block, 0)?) else {
        return Err("default-materialize:install-path".into());
    };
    if path_name_from_expr(&some.func) != "Some" || some.args.len() != 1 {
        return Err("default-materialize:install-path".into());
    }
    some.args
        .first()
        .ok_or_else(|| "default-materialize:install-path".into())
}

fn require_install_path_joins(mut current: &syn::Expr) -> Result<&syn::Expr, String> {
    for expected in [
        "mathjax-runtime.min.js",
        "generated",
        "katana-render-runtime",
    ] {
        let syn::Expr::MethodCall(join) = current else {
            return Err("default-materialize:install-path-join".into());
        };
        if join.method != "join"
            || join.args.len() != 1
            || !matches!(join.args.first(), Some(syn::Expr::Lit(value)) if matches!(&value.lit, syn::Lit::Str(text) if text.value() == expected))
        {
            return Err("default-materialize:install-path-join".into());
        }
        current = &join.receiver;
    }
    Ok(current)
}

fn require_temp_dir_root(current: &syn::Expr) -> Result<(), String> {
    let Some(temp_dir) = call(current) else {
        return Err("default-materialize:install-path-temp-dir".into());
    };
    if path_name_from_expr(&temp_dir.func) != "std::env::temp_dir" || !temp_dir.args.is_empty() {
        return Err("default-materialize:install-path-temp-dir".into());
    }
    Ok(())
}
