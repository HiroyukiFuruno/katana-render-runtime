use super::test_support::{Fixture, repository_root};
use super::*;
use std::fs;

const MATHJAX_RENDERER_INDEX: usize = 0;
const MATHJAX_JS_RUNTIME_INDEX: usize = 1;
const MATHJAX_JS_SCRIPTS_INDEX: usize = 2;
const MATHJAX_RUNTIME_PATH_RESOLVER_INDEX: usize = 3;
const MATHJAX_BACKEND_INDEX: usize = 4;
const V8_SCRIPT_ORIGIN_HELPER_INDEX: usize = 6;
const LIB_MODULE_REGISTRATION_INDEX: usize = 7;
const RUNTIME_ASSET_HELPER_INPUT_INDEX: usize = 8;
const GENERATED_MATHJAX_RUNTIME_INDEX: usize = 3;

#[path = "materializer_shared_readback_tests.rs"]
mod shared_readback;

#[test]
fn mathjax_consumer_facts_ast_linter_rejects_changed_routes_and_opaque_bytes_are_not_rust()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = repository_root()?;
    let input_paths = RUST_INPUTS
        .iter()
        .chain(OPAQUE_INPUTS)
        .copied()
        .collect::<Vec<_>>();
    let fixture = Fixture::copy_mathjax_inputs(&input_paths)?;
    assert_eq!(inspect_root(fixture.root())?["status"], "verified");
    reject_runtime_resolver_changes(&fixture, &repository)?;
    reject_default_materialization_changes(&fixture, &repository)?;
    reject_backend_and_source_changes(&fixture, &repository)?;
    reject_v8_and_registration_changes(&fixture, &repository)?;
    reject_parser_and_bundle_changes(&fixture, &repository)?;
    verify_opaque_input_is_not_parsed_as_rust(&fixture)?;
    Ok(())
}

fn reject_runtime_resolver_changes(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    reject_resolver_arm_and_dispatch(fixture, repository)?;
    reject_resolver_error_and_early_return(fixture, repository)
}

fn reject_resolver_arm_and_dispatch(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolver = RUST_INPUTS[MATHJAX_RUNTIME_PATH_RESOLVER_INDEX];
    let original = repository.join(resolver);
    fixture.replace(
        resolver,
        "DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),",
        "DiagramKind::MathJax => FakeMathJaxOps::resolve_mathjax_js(),",
    )?;
    fixture.reject("runtime-path-resolver:arm:DiagramKind::MathJax")?;
    fixture.restore_from(resolver, &original)?;

    fixture.replace(resolver, "match kind {", "match runtime_path {")?;
    fixture.reject("runtime-path-resolver:dispatch-shape")?;
    fixture.restore_from(resolver, &original)?;
    Ok(())
}

fn reject_resolver_error_and_early_return(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolver = RUST_INPUTS[MATHJAX_RUNTIME_PATH_RESOLVER_INDEX];
    let original = repository.join(resolver);
    fixture.replace(
        resolver,
        "map_err(RenderError::RuntimeResolution)",
        "map_err(ForgedError::RuntimeResolution)",
    )?;
    fixture.reject("runtime-path-resolver:map-err")?;
    fixture.restore_from(resolver, &original)?;

    fixture.replace(
        resolver,
        "if let Some(path) = runtime_path {",
        "if let Some(path) = None {",
    )?;
    fixture.reject("runtime-path-resolver:early-return")?;
    fixture.restore_from(resolver, &original)?;
    Ok(())
}

fn reject_default_materialization_changes(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let renderer = RUST_INPUTS[MATHJAX_RENDERER_INDEX];
    let original = repository.join(renderer);
    fixture.replace(
            renderer,
            "MathJaxGeneratedRuntimeAsset::materialize_at(path)",
            "if false { MathJaxGeneratedRuntimeAsset::materialize_at(path) } else { Err(\"x\".to_string()) }",
        )?;
    fixture.reject("default-materialize:callee:MathJaxGeneratedRuntimeAsset::materialize_at")?;
    fixture.restore_from(renderer, &original)?;

    fixture.replace(
        renderer,
        ".join(\"mathjax-runtime.min.js\")",
        ".join(\"forged.js\")",
    )?;
    fixture.reject("default-materialize:install-path-join")?;
    fixture.restore_from(renderer, &original)?;
    reject_materializer_implementation_changes(fixture, &original, renderer)?;
    Ok(())
}

fn reject_materializer_implementation_changes(
    fixture: &Fixture,
    original: &Path,
    renderer: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    fixture.replace(
        renderer,
        "    fn materialize_at(path: PathBuf) -> Result<PathBuf, String> {\n        if Self::exists_with_same_bytes(&path)? {\n            return Ok(path);\n        }\n        let Some(parent) = path.parent() else {\n            return Err(\"MathJax generated runtime path has no parent\".to_string());\n        };\n        std::fs::create_dir_all(parent).map_err(runtime_asset_error)?;\n        std::fs::write(&path, MathJaxRuntimeScripts::runtime_source())\n            .map_err(runtime_asset_error)?;\n        Ok(path)\n    }",
        "    fn materialize_at(_path: PathBuf) -> Result<PathBuf, String> {\n        Ok(PathBuf::from(\"not-the-generated-runtime.js\"))\n    }",
    )?;
    fixture.reject("default-materializer:materialize-signature")?;
    fixture.restore_from(renderer, original)?;
    fixture.replace(
        renderer,
        "    fn materialize_at(path: PathBuf) -> Result<PathBuf, String> {\n        if Self::exists_with_same_bytes(&path)? {\n            return Ok(path);\n        }\n        let Some(parent) = path.parent() else {\n            return Err(\"MathJax generated runtime path has no parent\".to_string());\n        };\n        std::fs::create_dir_all(parent).map_err(runtime_asset_error)?;\n        std::fs::write(&path, MathJaxRuntimeScripts::runtime_source())\n            .map_err(runtime_asset_error)?;\n        Ok(path)\n    }",
        "    fn materialize_at(path: PathBuf) -> Result<PathBuf, String> {\n        Ok(PathBuf::from(\"fake.js\"))\n    }",
    )?;
    fixture.reject("default-materializer:statement-count")?;
    fixture.restore_from(renderer, original)?;
    shared_readback::legacy_readback(fixture, renderer)?;
    fixture.replace(
        renderer,
        "struct MathJaxGeneratedRuntimeAsset;\n\nimpl MathJaxGeneratedRuntimeAsset {\n    fn materialize_at(path: PathBuf) -> Result<PathBuf, String> {\n        if Self::exists_with_same_bytes(&path)? {\n            return Ok(path);\n        }\n        let Some(parent) = path.parent() else {\n            return Err(\"MathJax generated runtime path has no parent\".to_string());\n        };\n        std::fs::create_dir_all(parent).map_err(runtime_asset_error)?;\n        std::fs::write(&path, MathJaxRuntimeScripts::runtime_source())\n            .map_err(runtime_asset_error)?;\n        Ok(path)\n    }\n\n    fn exists_with_same_bytes(path: &Path) -> Result<bool, String> {\n        match std::fs::read(path) {\n            Ok(existing) => Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes()),\n            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),\n            Err(error) => Err(runtime_asset_error(error)),\n        }\n    }\n}",
        "#[allow(non_snake_case)]\nmod MathJaxGeneratedRuntimeAsset {\n    use std::path::PathBuf;\n\n    pub(super) fn materialize_at(_path: PathBuf) -> Result<PathBuf, String> {\n        Ok(PathBuf::from(\"fake.js\"))\n    }\n}",
    )?;
    fixture.reject("default-materializer:missing-struct")?;
    fixture.restore_from(renderer, original)?;
    Ok(())
}

fn reject_backend_and_source_changes(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let backend = RUST_INPUTS[MATHJAX_BACKEND_INDEX];
    let original = repository.join(backend);
    fixture.replace(
        backend,
        "match self.render_block(input) {",
        "match Ok(String::new()) {",
    )?;
    fixture.reject("backend-runtime-path:render-dispatch")?;
    fixture.restore_from(backend, &original)?;

    let js_runtime = RUST_INPUTS[MATHJAX_JS_RUNTIME_INDEX];
    let original = repository.join(js_runtime);
    fixture.replace(
        js_runtime,
        "std::fs::read_to_string(mathjax_js)",
        "std::fs::read(mathjax_js)",
    )?;
    fixture.reject("read-render-route:source-reader-call")?;
    fixture.restore_from(js_runtime, &original)?;
    Ok(())
}

fn reject_v8_and_registration_changes(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let v8_helper = RUST_INPUTS[V8_SCRIPT_ORIGIN_HELPER_INDEX];
    let original = repository.join(v8_helper);
    fixture.replace(v8_helper, "origin_name.into(),", "source.into(),")?;
    fixture.reject("v8-script-origin-helper:origin")?;
    fixture.restore_from(v8_helper, &original)?;

    let registration = RUST_INPUTS[LIB_MODULE_REGISTRATION_INDEX];
    let original = repository.join(registration);
    fixture.replace(registration, "mod v8_script;", "pub mod v8_script;")?;
    fixture.reject("v8-script-origin-helper:module-registration")?;
    fixture.restore_from(registration, &original)?;
    Ok(())
}

fn reject_parser_and_bundle_changes(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    fixture.replace("Cargo.lock", "version = \"3.0.9\"", "version = \"3.99.0\"")?;
    fixture.reject("parser-pin:build-input-drift")?;
    fixture.restore_from("Cargo.lock", &repository.join("Cargo.lock"))?;

    let scripts = RUST_INPUTS[MATHJAX_JS_SCRIPTS_INDEX];
    let original = repository.join(scripts);
    fixture.replace(
        scripts,
        "../diagram_runtime/generated/mathjax-runtime.min.js",
        "../diagram_runtime/generated/forged.js",
    )?;
    fixture.reject("include-str-generated:path")?;
    fixture.restore_from(scripts, &original)?;
    Ok(())
}

fn verify_opaque_input_is_not_parsed_as_rust(
    fixture: &Fixture,
) -> Result<(), Box<dyn std::error::Error>> {
    let opaque = fixture
        .root()
        .join(OPAQUE_INPUTS[GENERATED_MATHJAX_RUNTIME_INDEX]);
    fs::write(opaque, b"not a Rust source file")?;
    assert_eq!(inspect_root(fixture.root())?["status"], "verified");
    Ok(())
}

#[test]
fn mathjax_shared_readback_rejects_inactive_and_alternate_owner_contexts()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = shared_readback::fixture()?;
    shared_readback::reject_owner_context_variants(
        &fixture,
        &[
            (
                RUST_INPUTS[0],
                "MathJaxGeneratedRuntimeAsset",
                "fn materialize_at(path: PathBuf) -> Result<PathBuf, String> { Ok(path) }",
                "default-materializer:owner-context",
            ),
            (
                RUST_INPUTS[RUNTIME_ASSET_HELPER_INPUT_INDEX],
                "RuntimeAsset",
                "pub(super) fn exists_with_matching_bytes(_path: &Path, _matches: impl FnOnce(&[u8]) -> Result<bool, String>) -> Result<bool, String> { Ok(true) }",
                "helper-binding-signature",
            ),
            (
                RUST_INPUTS[0],
                "MathJaxRendererOps",
                "pub fn default_install_path() -> Option<PathBuf> { None }",
                "renderer-owner-context",
            ),
        ],
    )?;
    reject_disabled_owner_binding(&fixture)?;
    reject_inactive_source_bindings(&fixture)?;
    Ok(())
}

fn reject_disabled_owner_binding(fixture: &Fixture) -> Result<(), Box<dyn std::error::Error>> {
    let generated_fake = "fn materialize_at(path: PathBuf) -> Result<PathBuf, String> { Ok(path) }";
    fixture.replace(
        RUST_INPUTS[0],
        "fn materialize_at(",
        "#[cfg(any())] fn materialize_at(",
    )?;
    fixture.reject("materialize-signature")?;
    fixture.replace(
        RUST_INPUTS[0],
        "    fn exists_with_same_bytes(",
        &format!("    {generated_fake}\n    fn exists_with_same_bytes("),
    )?;
    fixture.reject("default-materializer:owner-context")?;
    fixture.restore(RUST_INPUTS[0])?;
    inspect_root(fixture.root())?;
    Ok(())
}

fn reject_inactive_source_bindings(fixture: &Fixture) -> Result<(), Box<dyn std::error::Error>> {
    shared_readback::reject_mutations(
        fixture,
        RUST_INPUTS[0],
        &[(
            "mod js_runtime;",
            "#![cfg(any())]\nmod js_runtime;",
            "renderer-owner-context",
        )],
    )?;
    shared_readback::reject_mutations(
        fixture,
        RUST_INPUTS[RUNTIME_ASSET_HELPER_INPUT_INDEX],
        &[(
            "#[cfg(test)]\npub(super) use crate::markdown::runtime_asset_archive::ZENUML_RUNTIME_ASSETS_UNCOMPRESSED_LENGTH;\nuse crate::markdown::runtime_asset_archive::{self, RuntimeAssetSource};",
            "#![cfg(any())]\n#[cfg(test)]\npub(super) use crate::markdown::runtime_asset_archive::ZENUML_RUNTIME_ASSETS_UNCOMPRESSED_LENGTH;\nuse crate::markdown::runtime_asset_archive::{self, RuntimeAssetSource};",
            "helper-binding-signature",
        )],
    )
}
