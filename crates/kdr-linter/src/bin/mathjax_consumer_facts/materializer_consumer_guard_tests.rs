use super::{RUST_INPUTS, fixture, inspect_root, legacy_readback, reject_mutations};

const OWNER_DECLARATION_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "struct MathJaxGeneratedRuntimeAsset;",
        "",
        "default-materializer:missing-struct",
    ),
    (
        "struct MathJaxGeneratedRuntimeAsset;",
        "struct MathJaxGeneratedRuntimeAsset;\nstruct MathJaxGeneratedRuntimeAsset;",
        "default-materializer:ambiguous-struct",
    ),
    (
        "struct MathJaxGeneratedRuntimeAsset;",
        "pub struct MathJaxGeneratedRuntimeAsset;",
        "default-materializer:asset-type",
    ),
    (
        "struct MathJaxGeneratedRuntimeAsset;",
        "#[derive(Clone)]\nstruct MathJaxGeneratedRuntimeAsset;",
        "default-materializer:asset-type",
    ),
    (
        "struct MathJaxGeneratedRuntimeAsset;",
        "struct MathJaxGeneratedRuntimeAsset { marker: () }",
        "default-materializer:asset-type",
    ),
    (
        "struct MathJaxGeneratedRuntimeAsset;",
        "struct MathJaxGeneratedRuntimeAsset<T>;",
        "default-materializer:asset-type",
    ),
];

const READBACK_BODY_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "match std::fs::read(path)",
        "match std::fs::read(other_path)",
        "default-materializer:read-shape",
    ),
    (
        "Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes())",
        "Ok(existing != MathJaxRuntimeScripts::runtime_source().as_bytes())",
        "default-materializer:read-bytes-match",
    ),
    (
        "error.kind() == std::io::ErrorKind::NotFound",
        "error.kind() == std::io::ErrorKind::PermissionDenied",
        "default-materializer:not-found-match",
    ),
    (
        "Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false)",
        "Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true)",
        "default-materializer:not-found-match",
    ),
    (
        "Err(error) => Err(runtime_asset_error(error))",
        "Err(error) => Ok(false)",
        "default-materializer:read-error-match",
    ),
];

#[test]
fn rejects_noncanonical_asset_owner_declarations() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    inspect_root(fixture.root())?;
    reject_mutations(&fixture, RUST_INPUTS[0], OWNER_DECLARATION_MUTATIONS)
}

#[test]
fn rejects_readback_branch_shape_and_result_changes() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    inspect_root(fixture.root())?;
    for (before, after, expected) in READBACK_BODY_MUTATIONS {
        legacy_readback(&fixture, RUST_INPUTS[0])?;
        fixture.replace(RUST_INPUTS[0], before, after)?;
        fixture.reject(expected)?;
        fixture.restore(RUST_INPUTS[0])?;
        inspect_root(fixture.root())?;
    }
    Ok(())
}
