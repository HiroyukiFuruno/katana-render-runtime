use super::INPUTS;

const SVG_ID: &str = INPUTS[1];
const HASHER_STATEMENT: &str = "let mut hasher = DefaultHasher::new();";
const DOMAIN_STATEMENT: &str = "\"mermaid-svg-id-v1\".hash(&mut hasher);";
const SHARED_HASH_CALL: &str =
    "super::hash_mermaid_source_and_preset(&mut hasher, source, preset);";

pub(super) const MUTATIONS: &[(&str, &str, &str, &str)] = &[
    (
        SVG_ID,
        "fn svg_id(source: &str, preset: &DiagramColorPreset) -> String",
        "fn svg_id(source: &str, preset: &DiagramColorPreset, _namespace: &str) -> String",
        "svg-id-hash-route:signature",
    ),
    (
        SVG_ID,
        HASHER_STATEMENT,
        "std::mem::drop(DefaultHasher::new());",
        "svg-id-hash-route:hasher",
    ),
    (
        SVG_ID,
        HASHER_STATEMENT,
        "let mut hasher: DefaultHasher = DefaultHasher::new();",
        "svg-id-hash-route:hasher",
    ),
    (
        SVG_ID,
        HASHER_STATEMENT,
        "let mut hasher;",
        "svg-id-hash-route:hasher",
    ),
    (
        SVG_ID,
        HASHER_STATEMENT,
        "let mut hasher = { DefaultHasher::new() };",
        "svg-id-hash-route:hasher",
    ),
    (
        SVG_ID,
        DOMAIN_STATEMENT,
        "let _ = \"mermaid-svg-id-v1\";",
        "svg-id-hash-route:domain",
    ),
    (
        SVG_ID,
        DOMAIN_STATEMENT,
        "\"mermaid-svg-id-v1\".to_string().hash(&mut hasher);",
        "svg-id-hash-route:domain",
    ),
    (
        SVG_ID,
        SHARED_HASH_CALL,
        "let _ = super::hash_mermaid_source_and_preset(&mut hasher, source, preset);",
        "svg-id-hash-route:shared-fields",
    ),
    (
        SVG_ID,
        SHARED_HASH_CALL,
        "source.trim();",
        "svg-id-hash-route:shared-fields",
    ),
    (
        SVG_ID,
        "format!(\"katana-mermaid-svg-{:016x}\", hasher.finish())",
        "format!(\"katana-mermaid-svg-{:016x}\", 0_u64)",
        "svg-id-hash-route:output",
    ),
];
