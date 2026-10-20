use super::{
    MATHJAX_RENDERER_INDEX, RUST_INPUTS, inspect_root, mathjax_fixture,
    reject_generated_include_path_change, reject_parser_pin_change, repository_root,
};
use std::fs;

#[test]
fn mathjax_consumer_facts_ast_linter_rejects_non_utf8_rust_source()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let renderer = fixture.root().join(RUST_INPUTS[MATHJAX_RENDERER_INDEX]);
    fs::write(renderer, [0xff])?;
    let Err(error) = inspect_root(fixture.root()) else {
        return Err("invalid UTF-8 Rust source was accepted".into());
    };
    assert_eq!(
        error,
        format!("invalid-utf8:{}", RUST_INPUTS[MATHJAX_RENDERER_INDEX])
    );
    Ok(())
}

#[test]
fn mathjax_consumer_facts_ast_linter_rejects_parser_pin_and_generated_include_drift()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = repository_root()?;
    let fixture = mathjax_fixture()?;
    assert_eq!(inspect_root(fixture.root())?["status"], "verified");
    reject_parser_pin_change(&fixture, &repository)?;
    reject_generated_include_path_change(&fixture, &repository)?;
    assert_eq!(inspect_root(fixture.root())?["status"], "verified");
    Ok(())
}
