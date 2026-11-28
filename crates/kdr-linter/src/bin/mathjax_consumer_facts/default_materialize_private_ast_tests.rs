#![cfg(test)]

use super::{
    require_env_materialization_branch, require_environment_override,
    require_nonempty_environment_path,
};
use crate::source::parsed_block;

#[test]
fn env_materialization_requires_the_path_binding() -> Result<(), String> {
    let block = parsed_block(
        "fn route() { if let Some(other) = env_mathjax_js_from(env_value)? { return Ok(path); } }",
        "route",
    )?;
    assert_eq!(
        require_env_materialization_branch(&block).err().as_deref(),
        Some("default-materialize:env-branch")
    );
    Ok(())
}

#[test]
fn environment_override_requires_three_statements() -> Result<(), String> {
    let block = parsed_block(
        "fn route() { let Some(path) = value else { return Ok(None); }; if path.is_empty() { return Err(\"MATHJAX_JS is empty\".to_string()); } }",
        "route",
    )?;
    assert_eq!(
        require_environment_override(&block).err().as_deref(),
        Some("explicit-override:env-statement-count")
    );
    Ok(())
}

#[test]
fn environment_path_must_use_the_empty_check() -> Result<(), String> {
    let block = parsed_block(
        "fn route() { let Some(path) = value else { return Ok(None); }; if path.is_absolute() { return Err(\"MATHJAX_JS is empty\".to_string()); } Some(path) }",
        "route",
    )?;
    assert_eq!(
        require_nonempty_environment_path(&block).err().as_deref(),
        Some("explicit-override:empty-check")
    );
    Ok(())
}
