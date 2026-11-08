use super::super::test_support::Fixture;
use super::{opaque_sha256, opaque_text, parsed_block, parsed_source, sha256, source};
use std::fs;

const INPUTS: &[&str] = &["Cargo.toml", "Cargo.lock", "crates/kdr-linter/Cargo.toml"];

#[test]
fn source_text_helpers_propagate_read_and_utf8_errors() -> Result<(), String> {
    let fixture = Fixture::copy_mathjax_inputs(INPUTS).map_err(|error| error.to_string())?;
    let root = fixture.root();
    fs::create_dir(root.join("read-directory")).map_err(|error| error.to_string())?;

    let Err(error) = opaque_sha256(root, "read-directory") else {
        return Err("opaque SHA-256 accepted a directory".into());
    };
    assert!(error.starts_with("read-source:read-directory:"));

    fs::write(root.join("invalid.utf8"), [0xff]).map_err(|error| error.to_string())?;
    let Err(error) = opaque_text(root, "invalid.utf8") else {
        return Err("opaque text accepted invalid UTF-8".into());
    };
    assert_eq!(error, "invalid-utf8:invalid.utf8");

    let bytes = fs::read(root.join("Cargo.toml")).map_err(|error| error.to_string())?;
    assert_eq!(opaque_sha256(root, "Cargo.toml")?, sha256(&bytes));
    Ok(())
}

#[test]
fn rust_source_helpers_propagate_parse_errors_and_parse_blocks() -> Result<(), String> {
    let fixture = Fixture::copy_mathjax_inputs(INPUTS).map_err(|error| error.to_string())?;
    let root = fixture.root();
    fs::write(root.join("invalid.rs"), "fn {").map_err(|error| error.to_string())?;

    let Err(error) = source(root, "invalid.rs") else {
        return Err("invalid Rust source was accepted".into());
    };
    assert!(error.starts_with("parse-source:invalid.rs:"));

    let Err(error) = parsed_source("fn {") else {
        return Err("test parser helper accepted invalid Rust".into());
    };
    assert!(error.starts_with("parse-source:"));

    let block = parsed_block("fn evaluate() {}", "evaluate")?;
    assert!(block.stmts.is_empty());
    Ok(())
}
