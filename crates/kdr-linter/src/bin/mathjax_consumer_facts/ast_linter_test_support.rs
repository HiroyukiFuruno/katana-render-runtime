pub(super) use super::super::test_support::Fixture;
pub(super) use super::{OPAQUE_INPUTS, RUST_INPUTS, inspect_root, shared_readback};

pub(super) const MATHJAX_RENDERER_INDEX: usize = 0;
pub(super) const MATHJAX_JS_RUNTIME_INDEX: usize = 1;
pub(super) const MATHJAX_JS_SCRIPTS_INDEX: usize = 2;
pub(super) const MATHJAX_BACKEND_INDEX: usize = 4;
pub(super) const RUNTIME_ASSET_HELPER_INPUT_INDEX: usize = 8;
pub(super) const GENERATED_MATHJAX_RUNTIME_INDEX: usize = 3;

#[path = "ast_linter_materialization_test_support.rs"]
mod materialization;
#[path = "ast_linter_source_mutation_tests.rs"]
mod source;

pub(super) use materialization::{mathjax_fixture, reject_default_materialization_changes};
pub(super) use source::{
    reject_backend_and_source_changes, reject_disabled_owner_binding,
    reject_generated_include_path_change, reject_inactive_source_bindings,
    reject_parser_pin_change, verify_opaque_input_is_not_parsed_as_rust,
};
