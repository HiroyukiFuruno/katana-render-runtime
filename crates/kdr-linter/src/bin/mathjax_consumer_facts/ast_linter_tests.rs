use super::test_support::{Fixture, repository_root};
use super::*;
#[path = "ast_linter_test_support.rs"]
mod support;
use support::{
    MATHJAX_JS_RUNTIME_INDEX, MATHJAX_JS_SCRIPTS_INDEX, MATHJAX_RENDERER_INDEX,
    RUNTIME_ASSET_HELPER_INPUT_INDEX, mathjax_fixture, reject_backend_and_source_changes,
    reject_default_materialization_changes, reject_disabled_owner_binding,
    reject_generated_include_path_change, reject_inactive_source_bindings,
    reject_parser_pin_change, verify_opaque_input_is_not_parsed_as_rust,
};

#[path = "runtime_v8_route_tests.rs"]
mod runtime_v8_route_tests;

#[path = "materializer_shared_readback_tests.rs"]
mod shared_readback;

#[path = "call_binding_tests.rs"]
mod call_binding_tests;

#[path = "backend_route_tests.rs"]
mod backend_route_tests;

#[path = "consumer_tests.rs"]
mod consumer_tests;

#[path = "bundle_execution_tests.rs"]
mod bundle_execution_tests;

#[path = "resource_snapshot_cache_constructor_tests.rs"]
mod resource_snapshot_cache_constructor_tests;

#[path = "resource_snapshot_cache_attribute_tests.rs"]
mod resource_snapshot_cache_attribute_tests;

#[path = "shared_resource_loader_mutations_tests.rs"]
pub(super) mod shared_resource_loader_mutations;

#[path = "input_source_tests.rs"]
mod input_source_tests;

#[test]
fn mathjax_consumer_facts_ast_linter_rejects_changed_routes_and_opaque_bytes_are_not_rust()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = repository_root()?;
    let fixture = mathjax_fixture()?;
    assert_eq!(inspect_root(fixture.root())?["status"], "verified");
    runtime_v8_route_tests::reject_runtime_resolver_changes(&fixture, &repository)?;
    reject_default_materialization_changes(&fixture, &repository)?;
    reject_backend_and_source_changes(&fixture, &repository)?;
    runtime_v8_route_tests::reject_v8_and_registration_changes(&fixture, &repository)?;
    verify_opaque_input_is_not_parsed_as_rust(&fixture)?;
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
