use super::{Fixture, native_width_hash};

const HASH_IMPORT_CASES: &[(&str, &str, &str, &str)] = &[
    (
        native_width_hash::HASH_INPUTS[0],
        "use std::hash::{Hash, Hasher};",
        "use std::hash::Hash;",
        "hash-fields:imports",
    ),
    (
        native_width_hash::HASH_INPUTS[0],
        "use std::hash::{Hash, Hasher};",
        "use std::hash::Hasher;",
        "hash-fields:imports",
    ),
    (
        native_width_hash::HASH_INPUTS[1],
        "use std::collections::hash_map::DefaultHasher;",
        "use std::collections::hash_map::OtherHasher;",
        "hash-route:imports",
    ),
    (
        native_width_hash::HASH_INPUTS[1],
        "use std::hash::{Hash, Hasher};",
        "use std::hash::Hash;",
        "hash-route:imports",
    ),
];

const HASH_SIGNATURE_CASES: &[(&str, &str, &str, &str)] = &[
    (
        native_width_hash::HASH_INPUTS[0],
        "    source: &str,\n    preset: &DiagramColorPreset,",
        "    source: &str,",
        "hash-fields:signature",
    ),
    (
        native_width_hash::HASH_INPUTS[0],
        "    hasher: &mut H,",
        "    other: &mut H,",
        "hash-fields:signature",
    ),
    (
        native_width_hash::HASH_INPUTS[0],
        "fn hash_mermaid_source_and_preset<H: Hasher>(",
        "fn hash_mermaid_source_and_preset<H>(",
        "hash-fields:signature",
    ),
    (
        native_width_hash::HASH_INPUTS[0],
        "fn hash_mermaid_source_and_preset<H: Hasher>(",
        "fn hash_mermaid_source_and_preset<const H: usize>(",
        "hash-fields:signature",
    ),
    (
        native_width_hash::HASH_INPUTS[0],
        "fn hash_mermaid_source_and_preset<H: Hasher>(",
        "fn hash_mermaid_source_and_preset<H: Hasher + Send>(",
        "hash-fields:signature",
    ),
];

const HASH_ROUTE_SHAPE_CASES: &[(&str, &str, &str, &str)] = &[
    (
        native_width_hash::HASH_INPUTS[0],
        "preset.arrow.hash(hasher);",
        "preset.arrow.hash(hasher);\n    let _ = ();",
        "hash-fields:signature-or-count",
    ),
    (
        native_width_hash::HASH_INPUTS[1],
        "super::hash_mermaid_source_and_preset(&mut hasher, source, preset);",
        "super::hash_mermaid_source_and_preset(&mut hasher, source, preset);\n        let _ = ();",
        "svg-id-hash-route:statement-count",
    ),
    (
        native_width_hash::HASH_INPUTS[1],
        "\"mermaid-svg-id-v1\".hash(&mut hasher);",
        "SVG_DOMAIN.hash(&mut hasher);",
        "svg-id-hash-route:domain",
    ),
];

#[test]
fn rejects_missing_hash_traits_and_noncanonical_hash_bounds()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_native_width_inputs(native_width_hash::HASH_INPUTS)?;
    native_width_hash::inspect_root(fixture.root())?;
    reject_cases(&fixture, HASH_IMPORT_CASES)?;
    reject_cases(&fixture, HASH_SIGNATURE_CASES)
}

fn reject_cases(
    fixture: &Fixture,
    cases: &[(&str, &str, &str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    for (path, original, replacement, expected) in cases {
        fixture.replace(path, original, replacement)?;
        fixture.reject(expected)?;
        fixture.restore(path)?;
        native_width_hash::inspect_root(fixture.root())?;
    }
    Ok(())
}

#[test]
fn rejects_uncovered_hash_route_statement_and_receiver_shapes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_native_width_inputs(native_width_hash::HASH_INPUTS)?;
    for (path, original, replacement, expected) in HASH_ROUTE_SHAPE_CASES {
        fixture.replace(path, original, replacement)?;
        assert_eq!(
            native_width_hash::inspect_root(fixture.root())
                .err()
                .as_deref(),
            Some(*expected)
        );
        fixture.restore(path)?;
    }
    Ok(())
}
