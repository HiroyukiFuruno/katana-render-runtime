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
