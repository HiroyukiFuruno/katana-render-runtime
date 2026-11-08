use super::super::{materializer, materializer_body, source};
use std::path::Path;

const EXISTING_GUARD_INDEX: usize = 0;
const PARENT_BINDING_INDEX: usize = 1;
const CREATE_PARENT_INDEX: usize = 2;

const BODY_SHAPE_CASES: &[(usize, &str, &str)] = &[
    (
        EXISTING_GUARD_INDEX,
        "let _existing = Self::exists_with_same_bytes(&path)?;",
        "default-materializer:existing-byte-guard",
    ),
    (
        EXISTING_GUARD_INDEX,
        "if Self::exists_with_same_bytes(&path) { return Ok(path); }",
        "default-materializer:existing-byte-guard",
    ),
    (
        EXISTING_GUARD_INDEX,
        "if path? { return Ok(path); }",
        "default-materializer:existing-byte-guard",
    ),
    (
        PARENT_BINDING_INDEX,
        "path.parent();",
        "default-materializer:parent-binding",
    ),
    (
        PARENT_BINDING_INDEX,
        "let parent;",
        "default-materializer:parent-binding",
    ),
    (
        CREATE_PARENT_INDEX,
        "std::fs::create_dir_all(parent).map_err(runtime_asset_error);",
        "default-materializer:create-parent",
    ),
    (
        CREATE_PARENT_INDEX,
        "std::fs::create_dir_all(parent)?;",
        "default-materializer:create-parent",
    ),
    (
        materializer::SOURCE_WRITE_INDEX,
        "std::fs::write(&path, MathJaxRuntimeScripts::runtime_source()).map_err(runtime_asset_error);",
        "default-materializer:write-source",
    ),
    (
        materializer::SOURCE_WRITE_INDEX,
        "std::fs::write(&path, MathJaxRuntimeScripts::runtime_source())?;",
        "default-materializer:write-source",
    ),
    (
        materializer::SOURCE_WRITE_INDEX,
        "write_result.map_err(runtime_asset_error)?;",
        "default-materializer:write-source",
    ),
    (
        materializer::RETURNED_PATH_INDEX,
        "let returned = Ok(path);",
        "default-materializer:return-path",
    ),
];

#[test]
fn rejects_statement_and_expression_shape_variants_from_real_source()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = actual_materialize_block()?;
    assert!(materializer_body::materialize_body(&baseline).is_ok());
    assert_eq!(
        baseline.stmts.len(),
        materializer::MATERIALIZE_STATEMENT_COUNT
    );
    for (index, replacement, expected) in BODY_SHAPE_CASES {
        let mut changed = baseline.clone();
        changed.stmts[*index] = syn::parse_str(replacement)?;
        assert_eq!(changed.stmts.len(), baseline.stmts.len());
        assert_eq!(
            materializer_body::materialize_body(&changed)
                .err()
                .as_deref(),
            Some(*expected)
        );
    }
    Ok(())
}

fn actual_materialize_block() -> Result<syn::Block, Box<dyn std::error::Error>> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("workspace root")?;
    let parsed =
        source::source(workspace, source::RUST_INPUTS[0]).map_err(std::io::Error::other)?;
    let method = source::method(
        &parsed.ast,
        "MathJaxGeneratedRuntimeAsset",
        "materialize_at",
    )
    .map_err(std::io::Error::other)?;
    Ok(method.block.clone())
}

#[test]
fn unknown_owner_and_nested_runtime_source_are_checked() -> Result<(), syn::Error> {
    let file = syn::parse_file("struct Unknown;")?;
    assert!(!materializer::canonical_materializer_impl(&file, "Unknown"));
    for (expression, expected) in [
        ("sink(MathJaxRuntimeScripts::runtime_source())", true),
        ("sink(other())", false),
    ] {
        let expression = syn::parse_str(expression)?;
        assert_eq!(
            materializer_body::call_expr(
                &expression,
                "sink",
                &["MathJaxRuntimeScripts::runtime_source"]
            ),
            expected
        );
    }
    Ok(())
}

#[test]
fn default_install_path_rejects_invalid_shapes() -> Result<(), String> {
    let root = "other.join(\"katana-render-runtime\").join(\"generated\").join(\"mathjax-runtime.min.js\")";
    for (body, expected) in [
        (
            "let value = 1; Some(value)".to_string(),
            "default-materialize:install-path-statement-count",
        ),
        (
            "None(value)".to_string(),
            "default-materialize:install-path",
        ),
        (
            "Some(value)".to_string(),
            "default-materialize:install-path-join",
        ),
        (
            format!("Some({root})"),
            "default-materialize:install-path-temp-dir",
        ),
    ] {
        let block = source::parsed_block(&format!("fn fixture() {{ {body} }}"), "fixture")?;
        assert_eq!(
            super::super::default_route::require_default_install_path(&block)
                .err()
                .as_deref(),
            Some(expected)
        );
    }
    Ok(())
}
