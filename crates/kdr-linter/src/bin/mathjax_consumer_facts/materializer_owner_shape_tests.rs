use super::super::{HELPER, RUST_INPUTS, fixture};

const GENERATED_MATERIALIZER_SOURCE_INDEX: usize = 0;
const OWNER_SHAPE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "impl MathJaxGeneratedRuntimeAsset {",
        "impl<T> MathJaxGeneratedRuntimeAsset {",
        "default-materializer:owner-context",
    ),
    (
        "impl MathJaxGeneratedRuntimeAsset {",
        "impl MathJaxGeneratedRuntimeAsset where Self: Sized {",
        "default-materializer:owner-context",
    ),
    (
        "impl MathJaxGeneratedRuntimeAsset {",
        "impl std::fmt::Debug for MathJaxGeneratedRuntimeAsset {",
        "default-materializer:owner-context",
    ),
    (
        "impl MathJaxGeneratedRuntimeAsset {",
        "impl MathJaxGeneratedRuntimeAsset { const EXTRA: () = ();",
        "default-materializer:owner-context",
    ),
    (
        "fn exists_with_same_bytes(",
        "fn exists_with_same_bytes_other(",
        "default-materializer:owner-context",
    ),
];
const FILE_ATTRIBUTE_MUTATION: (&str, &str, &str) = (
    "#[cfg(test)]\npub(super) use crate::markdown::runtime_asset_archive::ZENUML_RUNTIME_ASSETS_UNCOMPRESSED_LENGTH;",
    "#![forbid(unsafe_code)]\n#[cfg(test)]\npub(super) use crate::markdown::runtime_asset_archive::ZENUML_RUNTIME_ASSETS_UNCOMPRESSED_LENGTH;",
    "default-materializer:helper-binding-signature",
);

#[test]
fn actual_mathjax_sources_reject_uncovered_materializer_owner_shapes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    fixture.reject_unique_mutations(
        RUST_INPUTS[GENERATED_MATERIALIZER_SOURCE_INDEX],
        OWNER_SHAPE_MUTATIONS,
    )?;
    fixture.reject_unique_mutations(HELPER, std::slice::from_ref(&FILE_ATTRIBUTE_MUTATION))
}
