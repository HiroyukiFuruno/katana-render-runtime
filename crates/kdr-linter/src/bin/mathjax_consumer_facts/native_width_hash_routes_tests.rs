use super::{
    require_hash_literal, require_hash_statement, require_hasher_binding, require_shared_hash_call,
};
use syn::Stmt;

const HASH_ERROR: &str = "hash-contract";
const SHARED_ERROR: &str = "shared-hash-contract";
const DOMAIN_ERROR: &str = "hash-domain-contract";
const HASH_STATEMENT_NEGATIVES: &[(&str, &str)] = &[
    ("source.write_hash(&mut hasher);", "method"),
    ("other.hash(&mut hasher);", "receiver"),
    ("source.hash(other);", "hasher argument"),
    ("source.hash(&mut hasher, 1);", "argument count"),
    ("let unused = 1;", "non-expression"),
];
const HASHER_BINDING_NEGATIVES: &[&str] = &[
    "let hasher = OtherHasher::new();",
    "let hasher = DefaultHasher::new(1);",
    "let hasher = DefaultHasher;",
    "let hasher;",
    "let hasher = 7;",
];
const HASH_LITERAL_NEGATIVES: &[(&str, &str)] = &[
    ("source.hash(&mut hasher);", "expected-domain"),
    ("true.hash(&mut hasher);", "expected-domain"),
    ("DOMAIN.hash(&mut hasher);", "expected-domain"),
    (
        "\"expected-domain\".write_hash(&mut hasher);",
        "expected-domain",
    ),
    (
        "\"expected-domain\".hash(&mut hasher, 1);",
        "expected-domain",
    ),
];
const SHARED_CALL_NEGATIVES: &[(&str, &str)] = &[
    ("source;", "not a function call"),
    ("hash_fields(&mut hasher, source, preset);", "callee"),
    (
        "super::hash_mermaid_source_and_preset(&mut hasher, source);",
        "argument count",
    ),
    (
        "super::hash_mermaid_source_and_preset(&mut hasher, source, preset, true);",
        "argument count",
    ),
];

#[test]
fn hash_statement_rejects_receiver_method_and_argument_mismatches() -> Result<(), String> {
    for (source, label) in HASH_STATEMENT_NEGATIVES {
        let statement = parse_statement(source)?;
        assert!(
            require_hash_statement(&statement, "source", "hasher", HASH_ERROR).is_err(),
            "accepted {label} mismatch"
        );
    }
    Ok(())
}

#[test]
fn hasher_binding_requires_the_zero_argument_default_constructor() -> Result<(), String> {
    for source in HASHER_BINDING_NEGATIVES {
        let statement = parse_statement(source)?;
        assert_eq!(
            require_hasher_binding(&statement, HASH_ERROR)
                .err()
                .as_deref(),
            Some(HASH_ERROR)
        );
    }
    Ok(())
}

#[test]
fn hash_domain_requires_a_string_literal_hash_call() -> Result<(), String> {
    for (source, expected_domain) in HASH_LITERAL_NEGATIVES {
        let statement = parse_statement(source)?;
        assert_eq!(
            require_hash_literal(&statement, expected_domain, DOMAIN_ERROR)
                .err()
                .as_deref(),
            Some(DOMAIN_ERROR)
        );
    }
    Ok(())
}

#[test]
fn shared_hash_call_requires_the_named_function_and_three_inputs() -> Result<(), String> {
    for (source, label) in SHARED_CALL_NEGATIVES {
        let statement = parse_statement(source)?;
        assert!(
            require_shared_hash_call(&statement, SHARED_ERROR, &["hasher", "source", "preset"])
                .is_err(),
            "accepted shared-call {label} mismatch"
        );
    }
    Ok(())
}

#[test]
fn hash_literal_rejects_non_expression_statements() -> Result<(), String> {
    let statement = parse_statement("let value = 1;")?;
    assert_rejected(
        require_hash_literal(&statement, "mermaid-svg-id-v1", DOMAIN_ERROR),
        DOMAIN_ERROR,
    );
    Ok(())
}

#[test]
fn shared_hash_call_rejects_non_expression_statements() -> Result<(), String> {
    let statement = parse_statement("let value = 1;")?;
    assert_rejected(
        require_shared_hash_call(&statement, SHARED_ERROR, &["hasher", "source", "preset"]),
        SHARED_ERROR,
    );
    Ok(())
}

#[test]
fn hasher_binding_rejects_nonlocal_statements() -> Result<(), String> {
    let statement = parse_statement("return DefaultHasher::new();")?;
    assert_rejected(require_hasher_binding(&statement, HASH_ERROR), HASH_ERROR);
    Ok(())
}

#[test]
fn hasher_binding_requires_the_expected_local_name() -> Result<(), String> {
    let statement = parse_statement("let other = DefaultHasher::new();")?;
    assert_rejected(require_hasher_binding(&statement, HASH_ERROR), HASH_ERROR);
    Ok(())
}

fn assert_rejected(result: Result<(), String>, expected: &str) {
    assert_eq!(result.err().as_deref(), Some(expected));
}

fn parse_statement(source: &str) -> Result<Stmt, String> {
    syn::parse_str(source).map_err(|error| error.to_string())
}
