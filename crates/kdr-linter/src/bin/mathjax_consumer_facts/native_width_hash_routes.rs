use super::HASH_FIELDS;
use super::ast::{
    require_cache_path, require_cache_signature, require_format_return, require_svg_id_signature,
    statement_expression,
};
use super::source::{self, Source};
use syn::{Expr, Stmt};

const HASH_FIELD_PARAMETER_COUNT: usize = 3;
const SVG_ID_STATEMENT_COUNT: usize = 4;
const SVG_DOMAIN_STATEMENT: usize = 1;
const SVG_SHARED_FIELDS_STATEMENT: usize = 2;
const SVG_OUTPUT_STATEMENT: usize = 3;
const RENDER_CACHE_STATEMENT_COUNT: usize = 9;
const RENDER_CACHE_PREFIX_START: usize = 1;
const RENDER_CACHE_PREFIX_END: usize = 6;
const RENDER_CACHE_SHARED_FIELDS_STATEMENT: usize = 6;
const RENDER_CACHE_DARK_MODE_STATEMENT: usize = 7;
const RENDER_CACHE_PATH_STATEMENT: usize = 8;

pub(super) fn validate_hash_fields(source: &Source) -> Result<(), String> {
    let function = source::free_function(&source.ast, "hash_mermaid_source_and_preset")?;
    super::require_hash_signature(function)?;
    if function.sig.inputs.len() != HASH_FIELD_PARAMETER_COUNT
        || function.block.stmts.len() != HASH_FIELDS.len()
    {
        return Err("hash-fields:signature-or-count".into());
    }
    for (statement, field) in function.block.stmts.iter().zip(HASH_FIELDS) {
        require_hash_statement(statement, field, "hasher", "hash-fields:order")?;
    }
    Ok(())
}

pub(super) fn validate_svg_id(source: &Source) -> Result<(), String> {
    let method = source::method(&source.ast, "MermaidRenderRequest", "svg_id")?;
    require_svg_id_signature(method)?;
    let statements = &method.block.stmts;
    if statements.len() != SVG_ID_STATEMENT_COUNT {
        return Err("svg-id-hash-route:statement-count".into());
    }
    require_hasher_binding(&statements[0], "svg-id-hash-route:hasher")?;
    require_hash_literal(
        &statements[SVG_DOMAIN_STATEMENT],
        "mermaid-svg-id-v1",
        "svg-id-hash-route:domain",
    )?;
    require_shared_hash_call(
        &statements[SVG_SHARED_FIELDS_STATEMENT],
        "svg-id-hash-route:shared-fields",
        &["hasher", "source", "preset"],
    )?;
    require_format_return(
        &statements[SVG_OUTPUT_STATEMENT],
        "katana-mermaid-svg-{:016x}",
        "hasher.finish()",
        "svg-id-hash-route:output",
    )
}

pub(super) fn validate_render_cache(source: &Source) -> Result<(), String> {
    let method = source::method(
        &source.ast,
        "MermaidRenderOps",
        "cache_file_path_with_zenuml_key",
    )?;
    require_cache_signature(method)?;
    let statements = &method.block.stmts;
    if statements.len() != RENDER_CACHE_STATEMENT_COUNT {
        return Err("render-cache-hash-route:statement-count".into());
    }
    require_hasher_binding(&statements[0], "render-cache-hash-route:hasher")?;
    require_cache_prefix(&statements[RENDER_CACHE_PREFIX_START..RENDER_CACHE_PREFIX_END])?;
    require_shared_hash_call(
        &statements[RENDER_CACHE_SHARED_FIELDS_STATEMENT],
        "render-cache-hash-route:shared-fields",
        &["hasher", "source", "preset"],
    )?;
    require_hash_statement(
        &statements[RENDER_CACHE_DARK_MODE_STATEMENT],
        "preset.dark_mode",
        "hasher",
        "render-cache-hash-route:dark-mode",
    )?;
    require_cache_path(&statements[RENDER_CACHE_PATH_STATEMENT])
}

fn require_cache_prefix(statements: &[Stmt]) -> Result<(), String> {
    for (statement, field) in statements.iter().zip(super::CACHE_PREFIX) {
        require_hash_statement(
            statement,
            field,
            "hasher",
            &format!("render-cache-hash-route:prefix:{field}"),
        )?;
    }
    Ok(())
}

fn require_hash_statement(
    statement: &Stmt,
    receiver: &str,
    hasher: &str,
    error: &str,
) -> Result<(), String> {
    let Some(Expr::MethodCall(call)) = statement_expression(statement) else {
        return Err(error.into());
    };
    if call.method != "hash"
        || !hash_receiver_matches(&call.receiver, receiver)
        || call.args.len() != 1
        || !source::ident(&call.args[0], hasher)
    {
        return Err(format!(
            "{error}:method={},receiver={},args={}",
            call.method,
            source::path_name_from_expr(&call.receiver),
            call.args.len()
        ));
    }
    Ok(())
}

fn hash_receiver_matches(expression: &Expr, expected: &str) -> bool {
    if let Expr::MethodCall(call) = expression {
        return expected == "mode.mermaid_cache_profile"
            && call.method == "mermaid_cache_profile"
            && call.args.is_empty()
            && source::ident(&call.receiver, "mode");
    }
    source::path_name_from_expr(expression) == expected
}

fn require_hasher_binding(statement: &Stmt, error: &str) -> Result<(), String> {
    let Expr::Call(call) = local_initializer(statement, "hasher", error)? else {
        return Err(error.into());
    };
    if source::path_name_from_expr(&call.func) != "DefaultHasher::new" || !call.args.is_empty() {
        return Err(error.into());
    }
    Ok(())
}

fn require_hash_literal(statement: &Stmt, literal: &str, error: &str) -> Result<(), String> {
    let Some(Expr::MethodCall(call)) = statement_expression(statement) else {
        return Err(error.into());
    };
    let Expr::Lit(receiver) = call.receiver.as_ref() else {
        return Err(error.into());
    };
    if call.method != "hash"
        || !matches!(&receiver.lit, syn::Lit::Str(value) if value.value() == literal)
        || call.args.len() != 1
        || !source::ident(&call.args[0], "hasher")
    {
        return Err(error.into());
    }
    Ok(())
}

fn require_shared_hash_call(
    statement: &Stmt,
    error: &str,
    arguments: &[&str],
) -> Result<(), String> {
    let Some(expression) = statement_expression(statement) else {
        return Err(error.into());
    };
    let Some(call) = source::call(expression) else {
        return Err(error.into());
    };
    let actual = call
        .args
        .iter()
        .map(source::path_name_from_expr)
        .collect::<Vec<_>>();
    if source::path_name_from_expr(&call.func) != "super::hash_mermaid_source_and_preset"
        || actual != arguments
    {
        return Err(error.into());
    }
    Ok(())
}

fn local_initializer<'a>(statement: &'a Stmt, name: &str, error: &str) -> Result<&'a Expr, String> {
    let Stmt::Local(local) = statement else {
        return Err(error.into());
    };
    if !matches!(&local.pat, syn::Pat::Ident(pattern) if pattern.ident == name) {
        return Err(error.into());
    }
    local
        .init
        .as_ref()
        .map(|init| init.expr.as_ref())
        .ok_or_else(|| error.into())
}
