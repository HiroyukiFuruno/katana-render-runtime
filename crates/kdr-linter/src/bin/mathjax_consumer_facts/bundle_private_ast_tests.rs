#![cfg(test)]

use super::{generated_include, script_entries, validate_template};
use crate::source::{parsed_block, parsed_source};

#[test]
fn generated_include_requires_exactly_one_runtime_constant() -> Result<(), String> {
    let source = parsed_source("const OTHER: &str = \"runtime\";")?;
    assert_eq!(
        generated_include(&source).err().as_deref(),
        Some("include-str-generated:count")
    );
    Ok(())
}

#[test]
fn script_entries_reject_multistatement_and_nonmacro_bodies() -> Result<(), String> {
    for source in ["fn fixture() {}", "fn fixture() { let _value = 1; _value }"] {
        let body = parsed_block(source, "fixture")?;
        assert_eq!(
            script_entries(&body).err().as_deref(),
            Some("script-order:body")
        );
    }
    let expression = parsed_block("fn fixture() { scripts }", "fixture")?;
    assert_eq!(
        script_entries(&expression).err().as_deref(),
        Some("script-order:shape")
    );
    Ok(())
}

#[test]
fn invocation_template_requires_a_literal_expression() -> Result<(), String> {
    let source = parsed_source("fn fixture() { format!(request_json) }")?;
    let function = crate::source::free_function(&source.ast, "fixture")?;
    let Some(syn::Stmt::Expr(syn::Expr::Macro(formatted), _)) = function.block.stmts.first() else {
        return Err("fixture macro expression missing".into());
    };
    assert_eq!(
        validate_template(formatted, "expected").err().as_deref(),
        Some("global-invocation:template")
    );
    Ok(())
}
