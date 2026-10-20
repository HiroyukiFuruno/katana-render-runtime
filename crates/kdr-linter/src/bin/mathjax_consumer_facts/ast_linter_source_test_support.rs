use super::{
    Fixture, GENERATED_MATHJAX_RUNTIME_INDEX, MATHJAX_BACKEND_INDEX, MATHJAX_JS_RUNTIME_INDEX,
    MATHJAX_JS_SCRIPTS_INDEX, OPAQUE_INPUTS, RUNTIME_ASSET_HELPER_INPUT_INDEX, RUST_INPUTS,
    inspect_root, shared_readback,
};
use std::fs;
use std::path::Path;

pub(in super::super) fn reject_backend_and_source_changes(
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
        "runtime_source_from(mathjax_js)",
        "runtime_source_from(&other_path)",
    )?;
    fixture.reject("read-render-route:argument:mathjax_js")?;
    fixture.restore_from(js_runtime, &original)?;
    Ok(())
}

pub(in super::super) fn reject_parser_pin_change(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    fixture.replace("Cargo.lock", "version = \"3.0.9\"", "version = \"3.99.0\"")?;
    fixture.reject("parser-pin:build-input-drift")?;
    fixture.restore_from("Cargo.lock", &repository.join("Cargo.lock"))?;
    fixture.replace(
        "Cargo.toml",
        "syn = { version = \"3\", features = [\"full\", \"visit\"] }",
        "syn = { version = \"4\", features = [\"full\", \"visit\"] }",
    )?;
    fixture.reject("parser-pin:build-input-drift")?;
    fixture.restore_from("Cargo.toml", &repository.join("Cargo.toml"))?;
    Ok(())
}

pub(in super::super) fn reject_generated_include_path_change(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
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

pub(in super::super) fn verify_opaque_input_is_not_parsed_as_rust(
    fixture: &Fixture,
) -> Result<(), Box<dyn std::error::Error>> {
    let opaque = fixture
        .root()
        .join(OPAQUE_INPUTS[GENERATED_MATHJAX_RUNTIME_INDEX]);
    fs::write(opaque, b"not a Rust source file")?;
    assert_eq!(inspect_root(fixture.root())?["status"], "verified");
    Ok(())
}

pub(in super::super) fn reject_disabled_owner_binding(
    fixture: &Fixture,
) -> Result<(), Box<dyn std::error::Error>> {
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

pub(in super::super) fn reject_inactive_source_bindings(
    fixture: &Fixture,
) -> Result<(), Box<dyn std::error::Error>> {
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
