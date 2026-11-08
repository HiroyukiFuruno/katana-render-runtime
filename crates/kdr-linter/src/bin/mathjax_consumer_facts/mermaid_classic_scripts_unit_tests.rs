use super::{literal, verify_source_snapshots};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use syn::Expr;

const SOURCE_PATH: &str = "src/scripts.rs";
const EXTRA_PATH: &str = "Cargo.lock";
const ORIGINAL_SOURCE: &[u8] = b"fn render() { original(); }";
const UPDATED_SOURCE: &[u8] = b"fn render() { updated(); }";
const EXTRA_SOURCE: &[u8] = b"[package]\nname = \"fixture\"\n";

#[test]
fn string_literal_contract_accepts_only_string_expression_nodes() -> Result<(), syn::Error> {
    let cases = [
        ("\"text\"", Some("text")),
        ("r#\"raw text\"#", Some("raw text")),
        ("17", None),
        ("true", None),
        ("b\"bytes\"", None),
    ];
    for (source, expected) in cases {
        assert_literal_case(source, expected)?;
    }
    Ok(())
}

fn assert_literal_case(source: &str, expected: Option<&str>) -> Result<(), syn::Error> {
    let expression = syn::parse_str::<Expr>(source)?;
    let actual = literal(&expression).ok();
    assert_eq!(actual.as_deref(), expected, "expression: {source}");
    Ok(())
}

#[test]
fn source_snapshot_comparison_preserves_match_drift_and_missing_contracts() {
    assert_snapshot_case(ORIGINAL_SOURCE, Some(ORIGINAL_SOURCE), Ok(()));
    assert_snapshot_case(
        ORIGINAL_SOURCE,
        Some(UPDATED_SOURCE),
        Err("mermaid-classic:source-drift:src/scripts.rs".to_string()),
    );
    assert_snapshot_case(
        ORIGINAL_SOURCE,
        None,
        Err("mermaid-classic:source-drift:src/scripts.rs".to_string()),
    );
}

fn assert_snapshot_case(
    before_source: &[u8],
    after_source: Option<&[u8]>,
    expected: Result<(), String>,
) {
    let before = BTreeMap::from([(SOURCE_PATH, hash_source(before_source))]);
    let mut after = BTreeMap::from([(EXTRA_PATH, hash_source(EXTRA_SOURCE))]);
    if let Some(source) = after_source {
        after.insert(SOURCE_PATH, hash_source(source));
    }
    assert_eq!(verify_source_snapshots(&before, &after), expected);
}

fn hash_source(source: &[u8]) -> String {
    Sha256::digest(source)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn request_binding_requires_an_initialized_nonreference_local() -> Result<(), String> {
    for (source, expected) in [
        ("{ let request_json = encode(); }", true),
        ("{ let mut request_json = encode(); }", true),
        ("{ let other = encode(); }", false),
        ("{ let ref request_json = encode(); }", false),
        ("{ let request_json; }", false),
        ("{ encode(); }", false),
        (
            "{ let Some(request_json) = encode() else { return; }; }",
            false,
        ),
    ] {
        let block = syn::parse_str::<syn::Block>(source).map_err(|error| error.to_string())?;
        assert_eq!(
            super::plain_local(&block, 0, "request_json").is_ok(),
            expected,
            "{source}"
        );
    }
    Ok(())
}

const SCRIPT_PUSH_CASES: [(&str, bool); 6] = [
    (
        "scripts.push(DiagramRuntimeScript::borrowed(\"main.js\", source));",
        true,
    ),
    (
        "other.push(DiagramRuntimeScript::borrowed(\"main.js\", source));",
        false,
    ),
    (
        "scripts.push(DiagramRuntimeScript::owned(\"main.js\", source));",
        false,
    ),
    (
        "scripts.push(DiagramRuntimeScript::borrowed(\"other.js\", source));",
        false,
    ),
    (
        "scripts.push(DiagramRuntimeScript::borrowed(\"main.js\", other));",
        false,
    ),
    (
        "scripts.push(DiagramRuntimeScript::borrowed(\"main.js\", source), extra);",
        false,
    ),
];

#[test]
fn script_push_keeps_the_named_source_and_borrowed_role() -> Result<(), String> {
    for (source, expected) in SCRIPT_PUSH_CASES {
        assert_push_case(source, expected)?;
    }
    assert!(super::push_entry(None, "main.js", "source", false).is_err());
    Ok(())
}

fn assert_push_case(source: &str, expected: bool) -> Result<(), String> {
    let statement = syn::parse_str::<syn::Stmt>(source).map_err(|error| error.to_string())?;
    assert_eq!(
        super::push_entry(Some(&statement), "main.js", "source", false).is_ok(),
        expected,
        "{source}"
    );
    Ok(())
}

#[test]
fn realm_macro_requires_the_expected_name_and_arguments() -> Result<(), String> {
    let statement =
        syn::parse_str::<syn::Stmt>("scope!(handle);").map_err(|error| error.to_string())?;
    assert!(super::macro_statement(Some(&statement), "scope!(handle);").is_ok());
    assert!(super::macro_statement(Some(&statement), "scope!(other);").is_err());
    assert!(super::macro_statement(Some(&statement), "other!(handle);").is_err());
    assert!(super::macro_statement(None, "scope!(handle);").is_err());
    Ok(())
}
