use super::super::consumer::require_direct_call;
use super::super::source::{Source, call, ident, local, method, record_fact};
use super::{INPUTS, REALM_INPUT_INDEX, macro_statement, scripts, tail};
use serde_json::Value;
use syn::{Expr, Stmt, spanned::Spanned};

const REALM: &str = "mermaid-classic-realm";
const CLASSIC_REALM_STATEMENTS: usize = 11;
const REALM_ISOLATE_STATEMENT_INDEX: usize = 1;
const REALM_SCOPE_MACRO_INDEX: usize = 2;
const REALM_CONTEXT_STATEMENT_INDEX: usize = 3;
const REALM_CONTEXT_SCOPE_STATEMENT_INDEX: usize = 4;
const REALM_TC_SCOPE_MACRO_INDEX: usize = 5;
const REALM_FONT_INSTALL_STATEMENT_INDEX: usize = 6;
const REALM_GEOMETRY_INSTALL_STATEMENT_INDEX: usize = 7;
const REALM_RENDERED_STATEMENT_INDEX: usize = 8;
const REALM_SCRIPT_LOOP_INDEX: usize = 9;

pub(super) fn inspect_realm(source: &Source, facts: &mut Vec<Value>) -> Result<(), String> {
    let render = method(&source.ast, "DiagramV8Runtime", "render")?;
    scripts::parameters(&render.sig, &["scripts"])?;
    let block = &render.block;
    if block.stmts.len() != CLASSIC_REALM_STATEMENTS {
        return Err("mermaid-classic-realm:statement-count".into());
    }
    initialize(block)?;
    context_scope(block)?;
    render_scripts(block)?;
    inspect_constructor(source, "borrowed", "Cow::Borrowed")?;
    inspect_constructor(source, "owned", "Cow::Owned")?;
    record_fact(
        facts,
        "mermaid-classic-shared-realm-loop",
        INPUTS[REALM_INPUT_INDEX],
        render.span(),
    );
    Ok(())
}

fn initialize(block: &syn::Block) -> Result<(), String> {
    let Some(Stmt::Expr(initialize, Some(_))) = block.stmts.first() else {
        return Err("mermaid-classic-realm:initialize".into());
    };
    require_direct_call(initialize, "Self::ensure_initialized", &[], REALM)?;
    default_allocation(
        local(block, REALM_ISOLATE_STATEMENT_INDEX, "isolate")?,
        "v8::Isolate::new",
        &[],
    )?;
    macro_statement(
        block.stmts.get(REALM_SCOPE_MACRO_INDEX),
        "v8::scope!(let handle_scope, &mut isolate);",
    )?;
    default_allocation(
        local(block, REALM_CONTEXT_STATEMENT_INDEX, "context")?,
        "v8::Context::new",
        &["handle_scope"],
    )
}

fn context_scope(block: &syn::Block) -> Result<(), String> {
    let Expr::Reference(reference) =
        local(block, REALM_CONTEXT_SCOPE_STATEMENT_INDEX, "context_scope")?
    else {
        return Err("mermaid-classic-realm:context-scope".into());
    };
    if reference.mutability.is_none() {
        return Err("mermaid-classic-realm:context-scope".into());
    }
    require_direct_call(
        &reference.expr,
        "v8::ContextScope::new",
        &["handle_scope", "context"],
        REALM,
    )?;
    macro_statement(
        block.stmts.get(REALM_TC_SCOPE_MACRO_INDEX),
        "v8::tc_scope!(let scope, &mut **context_scope);",
    )
}

fn render_scripts(block: &syn::Block) -> Result<(), String> {
    let Some(Stmt::Expr(install, Some(_))) = block.stmts.get(REALM_FONT_INSTALL_STATEMENT_INDEX)
    else {
        return Err("mermaid-classic-realm:install".into());
    };
    require_direct_call(install, "diagram_font_bridge::install", &["scope"], REALM)?;
    let Some(Stmt::Expr(install, Some(_))) =
        block.stmts.get(REALM_GEOMETRY_INSTALL_STATEMENT_INDEX)
    else {
        return Err("mermaid-classic-realm:geometry-install".into());
    };
    require_direct_call(
        install,
        "diagram_geometry_bridge::install",
        &["scope"],
        "mermaid-classic-realm:geometry-install",
    )?;
    require_direct_call(
        local(block, REALM_RENDERED_STATEMENT_INDEX, "rendered")?,
        "String::new",
        &[],
        REALM,
    )?;
    inspect_loop(block.stmts.get(REALM_SCRIPT_LOOP_INDEX))?;
    require_direct_call(tail(block)?, "Ok", &["rendered"], REALM)
}

fn default_allocation(expression: &Expr, callee: &str, prefix: &[&str]) -> Result<(), String> {
    let call = call(expression).ok_or("mermaid-classic-realm:allocation")?;
    if !ident(&call.func, callee)
        || call.args.len() != prefix.len() + 1
        || !call
            .args
            .iter()
            .zip(prefix)
            .all(|(arg, name)| ident(arg, name))
    {
        return Err("mermaid-classic-realm:allocation".into());
    }
    require_direct_call(
        call.args.last().ok_or("mermaid-classic-realm:allocation")?,
        "Default::default",
        &[],
        REALM,
    )
}

fn inspect_loop(statement: Option<&Stmt>) -> Result<(), String> {
    let Some(Stmt::Expr(Expr::ForLoop(loop_), None)) = statement else {
        return Err("mermaid-classic-realm:loop".into());
    };
    if loop_.label.is_some()
        || !ident(&loop_.expr, "scripts")
        || !matches!(loop_.pat.as_ref(), syn::Pat::Ident(binding) if binding.ident == "script" && binding.subpat.is_none())
        || loop_.body.stmts.len() != 1
    {
        return Err("mermaid-classic-realm:loop".into());
    }
    let Some(Stmt::Expr(Expr::Assign(assignment), Some(_))) = loop_.body.stmts.first() else {
        return Err("mermaid-classic-realm:evaluate".into());
    };
    if !ident(&assignment.left, "rendered") {
        return Err("mermaid-classic-realm:evaluate".into());
    }
    require_direct_call(&assignment.right, "evaluate", &["scope", "script"], REALM)
}

fn inspect_constructor(source: &Source, name: &str, cow: &str) -> Result<(), String> {
    let constructor = method(&source.ast, "DiagramRuntimeScript", name)?;
    scripts::parameters(&constructor.sig, &["name", "code"])?;
    let [Stmt::Expr(Expr::Struct(value), None)] = constructor.block.stmts.as_slice() else {
        return Err("mermaid-classic-realm:constructor".into());
    };
    if !value.path.is_ident("Self") || value.rest.is_some() || value.fields.len() != 2 {
        return Err("mermaid-classic-realm:constructor".into());
    }
    for (field, expected) in value.fields.iter().zip(["name", "code"]) {
        match (&field.member, expected) {
            (syn::Member::Named(member), "name")
                if member == "name" && ident(&field.expr, "name") => {}
            (syn::Member::Named(member), "code") if member == "code" => {
                require_direct_call(&field.expr, cow, &["code"], REALM)?
            }
            _ => return Err("mermaid-classic-realm:constructor".into()),
        }
    }
    Ok(())
}
