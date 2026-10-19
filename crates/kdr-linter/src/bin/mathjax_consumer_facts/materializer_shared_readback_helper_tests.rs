use super::{HELPER, fixture, inspect_root, reject_mutations};

const HELPER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "match std::fs::read(path)",
        "match std::fs::read(other_path)",
        "default-materializer:read-shape",
    ),
    (
        "Ok(existing) => matches(&existing)",
        "Ok(existing) => matches(&other)",
        "helper-callback-error",
    ),
    (
        "Ok(existing) => matches(&existing)",
        "Ok(existing) => { matches(&existing)?; matches(&existing) }",
        "helper-callback-error",
    ),
    (
        "Ok(existing) => matches(&existing)",
        "Ok(existing) => matches(&existing)?",
        "helper-callback-error",
    ),
    (
        "std::io::ErrorKind::NotFound => Ok(false)",
        "std::io::ErrorKind::NotFound => Ok(true)",
        "not-found-match",
    ),
    (
        "Err(error) => Err(error.to_string())",
        "Err(error) => Ok(false)",
        "helper-callback-error",
    ),
    (
        "impl FnOnce(&[u8])",
        "impl FnMut(&[u8])",
        "helper-binding-signature",
    ),
    (
        "path: &Path,\n        matches: impl FnOnce(&[u8]) -> Result<bool, String>",
        "path: &PathBuf,\n        matches: impl FnOnce(&[u8]) -> Result<bool, String>",
        "helper-binding-signature",
    ),
    (
        "path: &Path,\n        matches: impl FnOnce(&[u8]) -> Result<bool, String>",
        "path: &Path,\n        matches: Box<dyn FnOnce(&[u8]) -> Result<bool, String>>",
        "helper-binding-signature",
    ),
    (
        "path: &Path,\n        matches: impl FnOnce(&[u8]) -> Result<bool, String>",
        "path: &Path,\n        matches: impl FnOnce",
        "helper-binding-signature",
    ),
    (
        "matches: impl FnOnce(&[u8]) -> Result<bool, String>",
        "matches: impl FnOnce(&[u8], u8) -> Result<bool, String>",
        "helper-binding-signature",
    ),
    (
        "path: &Path,\n        matches: impl FnOnce(&[u8]) -> Result<bool, String>",
        "&self,\n        matches: impl FnOnce(&[u8]) -> Result<bool, String>",
        "helper-binding-signature",
    ),
    (
        "matches: impl FnOnce(&[u8]) -> Result<bool, String>",
        "matches: impl FnOnce(&[u8]) -> Result<u8, String>",
        "helper-binding-signature",
    ),
    (
        "match std::fs::read(path)",
        "return Ok(true);\n        match std::fs::read(path)",
        "helper-read-shape",
    ),
    (
        "pub(super) fn exists_with_matching_bytes(",
        "#[cfg(any())]\n    pub(super) fn exists_with_matching_bytes(",
        "helper-binding-signature",
    ),
    ("use std::{", "use forged::{", "helper-binding-signature"),
    (
        "use std::{",
        "use forged::*;\nuse std::{",
        "helper-binding-signature",
    ),
    (
        "pub(crate) struct RuntimeAsset {",
        "type RuntimeAsset = Forged;\npub(crate) struct RuntimeAsset {",
        "helper-binding-signature",
    ),
    (
        "path::{Path, PathBuf},",
        "path::{Path, PathBuf}, ffi::CString as Path,",
        "helper-binding-signature",
    ),
    (
        "use std::{",
        "#[cfg(any())] use std::{",
        "helper-binding-signature",
    ),
];

#[test]
fn mathjax_shared_readback_ast_linter_rejects_helper_mutations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    inspect_root(fixture.root())?;
    reject_mutations(&fixture, HELPER, HELPER_MUTATIONS)
}

#[test]
fn mathjax_shared_readback_accepts_only_the_test_asset_index_include_macro()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    let include = "#[cfg(test)]\ninclude!(\"generated/zenuml-runtime-assets-index.rs\");\n";
    fixture.replace(
        HELPER,
        "pub(crate) struct RuntimeAsset {",
        &format!("{include}pub(crate) struct RuntimeAsset {{"),
    )?;
    inspect_root(fixture.root())?;

    for replacement in [
        "include!(\"generated/zenuml-runtime-assets-index.rs\");\n",
        "#[cfg(any(test))]\ninclude!(\"generated/zenuml-runtime-assets-index.rs\");\n",
        "#[cfg(test)]\n#[allow(unused)]\ninclude!(\"generated/zenuml-runtime-assets-index.rs\");\n",
        "#[cfg(test)]\ninclude!(\"generated/other-runtime-assets-index.rs\");\n",
        "#[cfg(test)]\ninclude_str!(\"generated/zenuml-runtime-assets-index.rs\");\n",
    ] {
        fixture.replace(HELPER, include, replacement)?;
        fixture.reject("helper-binding-signature")?;
        fixture.replace(HELPER, replacement, include)?;
        inspect_root(fixture.root())?;
    }
    Ok(())
}
