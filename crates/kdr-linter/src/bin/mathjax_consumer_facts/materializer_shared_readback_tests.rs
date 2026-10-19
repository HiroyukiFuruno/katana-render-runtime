use super::{Fixture, OPAQUE_INPUTS, RUST_INPUTS, inspect_root};

#[path = "materializer_shared_readback_helper_tests.rs"]
mod helper_tests;

const HELPER: &str = "crates/katana-render-runtime/src/markdown/runtime_assets.rs";
const OWNER_CONTEXT_VARIANT_COUNT: usize = 5;
const SHARED: &str = "RuntimeAsset::exists_with_matching_bytes(path, |existing| {\n            Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes())\n        })";
const LEGACY: &str = "match std::fs::read(path) {\n            Ok(existing) => Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes()),\n            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),\n            Err(error) => Err(runtime_asset_error(error)),\n        }";
const RESERVED_TYPE_DECLARATIONS: &[&str] = &[
    "struct Path;",
    "enum Path { Forged }",
    "struct String;",
    "enum Result<T,E> { Ok(T), Err(E) }",
    "struct FnOnce;",
    "union Path { marker: () }",
    "extern crate forged as std;",
];

pub(super) fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
    let paths = RUST_INPUTS
        .iter()
        .chain(OPAQUE_INPUTS)
        .copied()
        .collect::<Vec<_>>();
    Fixture::copy_mathjax_inputs(&paths)
}

pub(super) fn legacy_readback(fixture: &Fixture, renderer: &str) -> std::io::Result<()> {
    fixture.replace(renderer, SHARED, LEGACY)
}

const CALLER_BINDING_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "use crate::markdown::runtime_assets::RuntimeAsset;",
        "use crate::markdown::runtime_assets::RuntimeAsset as Alias;",
        "shared-import-binding",
    ),
    (
        "struct MathJaxGeneratedRuntimeAsset;",
        "type RuntimeAsset = Forged;\nstruct MathJaxGeneratedRuntimeAsset;",
        "shared-import-binding",
    ),
    (
        "use crate::markdown::runtime_assets::RuntimeAsset;",
        "#[cfg(any())] use crate::markdown::runtime_assets::RuntimeAsset;",
        "shared-import-binding",
    ),
    (
        "struct MathJaxGeneratedRuntimeAsset;",
        "union RuntimeAsset { marker: () }\nstruct MathJaxGeneratedRuntimeAsset;",
        "shared-import-binding",
    ),
    (
        "struct MathJaxGeneratedRuntimeAsset;",
        "impl RuntimeAsset { fn exists_with_matching_bytes(_path: &Path, _matches: impl FnOnce(&[u8]) -> Result<bool, String>) -> Result<bool, String> { Ok(true) } }\nstruct MathJaxGeneratedRuntimeAsset;",
        "shared-import-binding",
    ),
    (
        "struct MathJaxGeneratedRuntimeAsset;",
        "impl RuntimeAsset {}\nstruct MathJaxGeneratedRuntimeAsset;",
        "shared-import-binding",
    ),
    (
        "use crate::markdown::runtime_assets::RuntimeAsset;",
        "#[cfg(any())] use crate::markdown::runtime_assets::RuntimeAsset;\nunion RuntimeAsset { marker: () }\nimpl RuntimeAsset { fn exists_with_matching_bytes(_path: &Path, _matches: impl FnOnce(&[u8]) -> Result<bool, String>) -> Result<bool, String> { Ok(true) } }",
        "shared-import-binding",
    ),
    (
        "use crate::markdown::runtime_assets::RuntimeAsset;",
        "use crate::markdown::runtime_assets::RuntimeAsset;\nextern crate forged as RuntimeAsset;",
        "shared-import-binding",
    ),
    (
        "struct MathJaxGeneratedRuntimeAsset;",
        "make_local_asset!();\nstruct MathJaxGeneratedRuntimeAsset;",
        "shared-import-binding",
    ),
    (
        "RuntimeAsset::exists_with_matching_bytes(path,",
        "RuntimeAsset::exists_with_matching_bytes(other_path,",
        "shared-call",
    ),
    (
        "RuntimeAsset::exists_with_matching_bytes(path, |existing| {\n            Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes())\n        })",
        "RuntimeAsset::exists_with_matching_bytes(path, |existing| {\n            Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes())\n        });",
        "shared-call-shape",
    ),
    (
        "RuntimeAsset::exists_with_matching_bytes(path, |existing| {\n            Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes())\n        })",
        "RuntimeAsset::exists_with_matching_bytes(path, true)",
        "shared-predicate",
    ),
    ("|existing| {", "|other| {", "shared-predicate"),
    (
        "Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes())",
        "Ok(true)",
        "shared-expected-bytes",
    ),
    (
        "|existing| {",
        "|existing| { return Ok(true);",
        "shared-predicate-body",
    ),
    (
        "RuntimeAsset::exists_with_matching_bytes(path, |existing| {\n            Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes())\n        })",
        "RuntimeAsset::exists_with_matching_bytes(path, |existing| Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes()))",
        "shared-predicate-body",
    ),
];
#[test]
fn mathjax_shared_readback_ast_linter_binds_current_and_legacy_routes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    for legacy in [false, true] {
        if legacy {
            legacy_readback(&fixture, RUST_INPUTS[0])?;
        }
        let report = inspect_root(fixture.root())?;
        assert_eq!(report["facts"].as_array().ok_or("facts")?.len(), 13);
        let pins = report["inputSha256"].as_object().ok_or("inputs")?;
        assert_eq!(pins.len(), 16);
        assert!(pins.contains_key(HELPER));
        assert_eq!(report["generatedGlobalVerified"], false);
        assert_eq!(report["overrideContentVerified"], false);
    }
    Ok(())
}

pub(super) fn reject_mutations(
    fixture: &Fixture,
    path: &str,
    cases: &[(&str, &str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    for (before, after, issue) in cases {
        fixture.replace(path, before, after)?;
        fixture.reject(issue)?;
        fixture.restore(path)?;
        inspect_root(fixture.root())?;
    }
    Ok(())
}

pub(super) fn reject_owner_context_variants(
    fixture: &Fixture,
    cases: &[(&str, &str, &str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    for (path, owner, fake, issue) in cases {
        let before = format!("impl {owner} {{");
        for after in owner_context_variants(owner, fake) {
            reject_mutations(fixture, path, &[(&before, &after, issue)])?;
        }
    }
    Ok(())
}

fn owner_context_variants(owner: &str, fake: &str) -> [String; OWNER_CONTEXT_VARIANT_COUNT] {
    [
        format!("#[cfg(any())] impl {owner} {{"),
        format!("impl self::{owner} {{"),
        format!("impl {owner} {{}}\nimpl {owner} {{"),
        format!("type AssetAlias = {owner};\nimpl AssetAlias {{ {fake} }}\nimpl {owner} {{"),
        format!("impl self::{owner} {{ {fake} }}\n#[cfg(any())] impl {owner} {{"),
    ]
}

#[test]
fn mathjax_shared_readback_ast_linter_rejects_caller_binding_mutations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    inspect_root(fixture.root())?;
    reject_mutations(&fixture, RUST_INPUTS[0], CALLER_BINDING_MUTATIONS)
}

#[test]
fn mathjax_shared_readback_ast_linter_rejects_missing_and_symlink_helper()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    let path = fixture.root().join(HELPER);
    let saved = path.with_extension("saved-rs");
    std::fs::rename(&path, &saved)?;
    assert!(inspect_root(fixture.root()).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&saved, &path)?;
        let result = inspect_root(fixture.root());
        assert!(result.is_err_and(|error| error.contains("unsafe-source-path")));
        std::fs::remove_file(&path)?;
    }
    std::fs::rename(saved, path)?;
    inspect_root(fixture.root())?;
    Ok(())
}

#[test]
fn mathjax_shared_readback_ast_linter_rejects_disabled_readback_and_reserved_types()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    for legacy in [false, true] {
        if legacy {
            legacy_readback(&fixture, RUST_INPUTS[0])?;
        }
        fixture.replace(
            RUST_INPUTS[0],
            "fn exists_with_same_bytes(",
            "#[cfg(any())] fn exists_with_same_bytes(",
        )?;
        fixture.reject("same-bytes-signature")?;
        fixture.restore(RUST_INPUTS[0])?;
        inspect_root(fixture.root())?;
    }
    for declaration in RESERVED_TYPE_DECLARATIONS {
        fixture.replace(
            HELPER,
            "pub(crate) struct RuntimeAsset {",
            &format!("{declaration}\npub(crate) struct RuntimeAsset {{"),
        )?;
        fixture.reject("helper-binding-signature")?;
        fixture.restore(HELPER)?;
        inspect_root(fixture.root())?;
    }
    Ok(())
}
