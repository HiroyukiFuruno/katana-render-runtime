use super::source::{RUST_INPUTS, Source, call, ident, record_fact};
use serde_json::Value;
use std::collections::BTreeMap;
use syn::spanned::Spanned;

const V8_SCRIPT_ORIGIN_ARG_COUNT: usize = 11;
const SCRIPT_COMPILE_ARG_COUNT: usize = 3;
const ORIGIN_SCOPE_INDEX: usize = 0;
const ORIGIN_NAME_INDEX: usize = 1;
const ORIGIN_ZERO_A_INDEX: usize = 2;
const ORIGIN_ZERO_B_INDEX: usize = 3;
const ORIGIN_FALSE_A_INDEX: usize = 4;
const ORIGIN_ZERO_C_INDEX: usize = 5;
const ORIGIN_NAME_INTO_INDEX: usize = 6;
const ORIGIN_FALSE_B_INDEX: usize = 7;
const ORIGIN_FALSE_C_INDEX: usize = 8;
const ORIGIN_FALSE_D_INDEX: usize = 9;
const ORIGIN_NONE_INDEX: usize = 10;
const DIAGRAM_JS_RUNTIME_INPUT_INDEX: usize = 5;
const V8_SCRIPT_INPUT_INDEX: usize = 6;
const LIBRARY_MODULE_INPUT_INDEX: usize = 7;

#[cfg(test)]
#[path = "v8_compile_tests.rs"]
mod tests;

pub(super) fn require_error_constants(
    file: &syn::File,
    expected: &[(&str, &str)],
) -> Result<(), String> {
    for (name, value) in expected {
        let found = file.items.iter().filter_map(|item| {
            let syn::Item::Const(item) = item else {
                return None;
            };
            (item.ident == *name).then_some(item.expr.as_ref())
        });
        if !found.into_iter().any(|expression| matches!(expression, syn::Expr::Lit(literal) if matches!(&literal.lit, syn::Lit::Str(text) if text.value() == *value))) {
            return Err("v8-compile-order:allocation-error".into());
        }
    }
    Ok(())
}

pub(super) fn inspect_compiler_helper(
    sources: &BTreeMap<&str, Source>,
    facts: &mut Vec<Value>,
) -> Result<(), String> {
    let helper_path = RUST_INPUTS[V8_SCRIPT_INPUT_INDEX];
    let registration_path = RUST_INPUTS[LIBRARY_MODULE_INPUT_INDEX];
    let helper = super::source::method(
        &sources[helper_path].ast,
        "ScriptCompiler",
        "compile_with_origin",
    )?;
    validate_origin_helper(helper)?;
    let module = private_module(&sources[registration_path].ast)?;
    require_compiler_import(&sources[RUST_INPUTS[DIAGRAM_JS_RUNTIME_INPUT_INDEX]].ast)?;
    record_fact(facts, "v8-script-origin-helper", helper_path, helper.span());
    record_fact(
        facts,
        "v8-script-private-module",
        registration_path,
        module.span(),
    );
    Ok(())
}

fn validate_origin_helper(helper: &syn::ImplItemFn) -> Result<(), String> {
    if helper.block.stmts.len() != 2 {
        return Err("v8-script-origin-helper:statement-count".into());
    }
    let origin = super::source::local(&helper.block, 0, "origin")?;
    require_origin_constructor(origin)?;
    let syn::Stmt::Expr(compiled, _) = &helper.block.stmts[1] else {
        return Err("v8-script-origin-helper:compile".into());
    };
    require_origin_compile(compiled)?;
    Ok(())
}

fn private_module(file: &syn::File) -> Result<&syn::ItemMod, String> {
    let modules = file
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Mod(module) = item else {
                return None;
            };
            (module.ident == "v8_script").then_some(module)
        })
        .collect::<Vec<_>>();
    match modules.as_slice() {
        [module]
            if matches!(module.vis, syn::Visibility::Inherited) && module.content.is_none() =>
        {
            Ok(module)
        }
        _ => Err("v8-script-origin-helper:module-registration".into()),
    }
}

fn require_compiler_import(file: &syn::File) -> Result<(), String> {
    let has_compiler_import = file
        .items
        .iter()
        .any(|item| {
            let syn::Item::Use(item) = item else {
                return false;
            };
            matches!(&item.tree, syn::UseTree::Path(root) if root.ident == "crate" && matches!(&*root.tree, syn::UseTree::Path(module) if module.ident == "v8_script" && matches!(&*module.tree, syn::UseTree::Name(name) if name.ident == "ScriptCompiler")))
        });
    if !has_compiler_import {
        return Err("v8-script-origin-helper:diagram-import".into());
    }
    Ok(())
}
fn require_origin_constructor(expression: &syn::Expr) -> Result<(), String> {
    let Some(constructor) = call(expression) else {
        return Err("v8-script-origin-helper:origin".into());
    };
    if super::source::path_name_from_expr(&constructor.func) != "v8::ScriptOrigin::new"
        || constructor.args.len() != V8_SCRIPT_ORIGIN_ARG_COUNT
        || !ident(&constructor.args[ORIGIN_SCOPE_INDEX], "scope")
        || !is_into(&constructor.args[ORIGIN_NAME_INDEX], "origin_name")
        || !is_integer(&constructor.args[ORIGIN_ZERO_A_INDEX], 0)
        || !is_integer(&constructor.args[ORIGIN_ZERO_B_INDEX], 0)
        || !is_boolean(&constructor.args[ORIGIN_FALSE_A_INDEX], false)
        || !is_integer(&constructor.args[ORIGIN_ZERO_C_INDEX], 0)
        || !is_some_into(&constructor.args[ORIGIN_NAME_INTO_INDEX], "origin_name")
        || !is_boolean(&constructor.args[ORIGIN_FALSE_B_INDEX], false)
        || !is_boolean(&constructor.args[ORIGIN_FALSE_C_INDEX], false)
        || !is_boolean(&constructor.args[ORIGIN_FALSE_D_INDEX], false)
        || !ident(&constructor.args[ORIGIN_NONE_INDEX], "None")
    {
        return Err("v8-script-origin-helper:origin".into());
    }
    Ok(())
}
fn require_origin_compile(expression: &syn::Expr) -> Result<(), String> {
    let Some(compile) = call(expression) else {
        return Err("v8-script-origin-helper:compile".into());
    };
    if super::source::path_name_from_expr(&compile.func) != "v8::Script::compile"
        || compile.args.len() != SCRIPT_COMPILE_ARG_COUNT
        || !ident(&compile.args[0], "scope")
        || !ident(&compile.args[1], "source")
        || !is_some_borrowed(&compile.args[2], "origin")
    {
        return Err("v8-script-origin-helper:compile".into());
    }
    Ok(())
}
fn is_into(expression: &syn::Expr, receiver: &str) -> bool {
    matches!(expression, syn::Expr::MethodCall(value) if value.method == "into" && value.args.is_empty() && ident(&value.receiver, receiver))
}
fn is_some_into(expression: &syn::Expr, receiver: &str) -> bool {
    let Some(some) = call(expression) else {
        return false;
    };
    super::source::path_name_from_expr(&some.func) == "Some"
        && some.args.len() == 1
        && is_into(&some.args[0], receiver)
}
fn is_some_borrowed(expression: &syn::Expr, receiver: &str) -> bool {
    let Some(some) = call(expression) else {
        return false;
    };
    super::source::path_name_from_expr(&some.func) == "Some"
        && some.args.len() == 1
        && matches!(&some.args[0], syn::Expr::Reference(value) if ident(&value.expr, receiver))
}
fn is_integer(expression: &syn::Expr, expected: u64) -> bool {
    matches!(expression, syn::Expr::Lit(value) if matches!(&value.lit, syn::Lit::Int(number) if number.base10_parse::<u64>().ok() == Some(expected)))
}
fn is_boolean(expression: &syn::Expr, expected: bool) -> bool {
    matches!(expression, syn::Expr::Lit(value) if matches!(&value.lit, syn::Lit::Bool(boolean) if boolean.value == expected))
}
