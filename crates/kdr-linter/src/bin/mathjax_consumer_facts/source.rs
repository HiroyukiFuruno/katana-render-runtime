use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub(super) const RESOURCE_BYTES_INPUT_INDEX: usize = 12;

pub(super) const RUST_INPUTS: &[&str] = &[
    "crates/katana-render-runtime/src/markdown/mathjax_renderer/mod.rs",
    "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime.rs",
    "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime_scripts.rs",
    "crates/katana-render-runtime/src/renderer/runtime_path.rs",
    "crates/katana-render-runtime/src/renderer/backends/mathjax.rs",
    "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs",
    "crates/katana-render-runtime/src/v8_script.rs",
    "crates/katana-render-runtime/src/lib.rs",
    "crates/katana-render-runtime/src/markdown/runtime_assets.rs",
    "crates/katana-render-runtime/src/markdown/runtime_resource_loader.rs",
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/render.rs",
    "crates/katana-render-runtime/src/renderer/backends/mermaid.rs",
    "crates/katana-render-runtime/src/resource_bytes.rs",
];
pub(super) const OPAQUE_INPUTS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "crates/kdr-linter/Cargo.toml",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/mathjax-runtime.min.js",
    "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime/render_mathjax.js",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mathjax/runtime_definition.ts",
    "scripts/runtime-bundles/runtime-bundle-definitions.ts",
];

const HEX_DIGIT_SHIFT: usize = 4;
const HEX_DIGIT_MASK: usize = 15;

pub(super) struct Source {
    pub(super) sha256: String,
    pub(super) ast: syn::File,
}

pub(super) use super::ast_lookup::{
    call, call_path, free_function, ident, local, method, path_name, path_name_from_expr,
    record_fact, trait_method,
};

pub(super) fn source(root: &Path, relative: &str) -> Result<Source, String> {
    let bytes = opaque_bytes(root, relative)?;
    let text = String::from_utf8(bytes.clone()).map_err(|_| format!("invalid-utf8:{relative}"))?;
    let sha256 = sha256(&bytes);
    let ast = syn::parse_file(&text).map_err(|error| format!("parse-source:{relative}:{error}"))?;
    Ok(Source { sha256, ast })
}

#[cfg(test)]
pub(super) fn parsed_source(text: &str) -> Result<Source, String> {
    let bytes = text.as_bytes();
    let sha256 = sha256(bytes);
    let ast = syn::parse_file(text).map_err(|error| format!("parse-source:{error}"))?;
    Ok(Source { sha256, ast })
}

#[cfg(test)]
pub(super) fn parsed_block(text: &str, function: &str) -> Result<syn::Block, String> {
    let source = parsed_source(text)?;
    Ok(free_function(&source.ast, function)?.block.as_ref().clone())
}

pub(super) fn opaque_sha256(root: &Path, relative: &str) -> Result<String, String> {
    Ok(sha256(&opaque_bytes(root, relative)?))
}

pub(super) fn opaque_text(root: &Path, relative: &str) -> Result<String, String> {
    String::from_utf8(opaque_bytes(root, relative)?).map_err(|_| format!("invalid-utf8:{relative}"))
}

pub(super) fn parser_pin(
    root: &Path,
    build_lock: &str,
    build_workspace: &str,
    build_linter: &str,
) -> Result<String, String> {
    let (workspace, linter, lock) =
        parser_manifests(root, build_lock, build_workspace, build_linter)?;
    validate_workspace_parser(&workspace)?;
    validate_linter_parser(&linter)?;
    locked_syn_version(&lock)
}

fn parser_manifests(
    root: &Path,
    build_lock: &str,
    build_workspace: &str,
    build_linter: &str,
) -> Result<(toml::Value, toml::Value, toml::Value), String> {
    let lock_text = opaque_text(root, "Cargo.lock")?;
    let workspace_text = opaque_text(root, "Cargo.toml")?;
    let linter_text = opaque_text(root, "crates/kdr-linter/Cargo.toml")?;
    if lock_text != build_lock || workspace_text != build_workspace || linter_text != build_linter {
        return Err("parser-pin:build-input-drift".into());
    }
    parse_parser_manifests(&workspace_text, &linter_text, &lock_text)
}

fn parse_parser_manifests(
    workspace_text: &str,
    linter_text: &str,
    lock_text: &str,
) -> Result<(toml::Value, toml::Value, toml::Value), String> {
    let workspace = toml::from_str(workspace_text)
        .map_err(|error| format!("parser-pin:workspace-manifest:{error}"))?;
    let linter = toml::from_str(linter_text)
        .map_err(|error| format!("parser-pin:linter-manifest:{error}"))?;
    let lock = toml::from_str(lock_text).map_err(|error| format!("parser-pin:lockfile:{error}"))?;
    Ok((workspace, linter, lock))
}

fn validate_workspace_parser(workspace: &toml::Value) -> Result<(), String> {
    let workspace_syn = workspace
        .get("workspace")
        .and_then(|value| value.get("dependencies"))
        .and_then(|value| value.get("syn"))
        .ok_or("parser-pin:workspace-syn")?;
    if workspace_syn.get("version").and_then(toml::Value::as_str) != Some("3")
        || string_array(workspace_syn.get("features"))?.as_slice() != ["full", "visit"]
    {
        return Err("parser-pin:workspace-features".into());
    }
    Ok(())
}

fn validate_linter_parser(linter: &toml::Value) -> Result<(), String> {
    let package_syn = linter
        .get("dependencies")
        .and_then(|value| value.get("syn"))
        .ok_or("parser-pin:linter-syn")?;
    if package_syn.get("workspace").and_then(toml::Value::as_bool) != Some(true) {
        return Err("parser-pin:linter-workspace-dependency".into());
    }
    Ok(())
}

fn locked_syn_version(lock: &toml::Value) -> Result<String, String> {
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("parser-pin:lock-packages")?;
    let versions = packages
        .iter()
        .filter(|package| package.get("name").and_then(toml::Value::as_str) == Some("syn"))
        .filter_map(|package| package.get("version").and_then(toml::Value::as_str))
        .filter(|version| version.starts_with("3."))
        .collect::<Vec<_>>();
    match versions.as_slice() {
        [version] => Ok((*version).to_string()),
        [] => Err("parser-pin:locked-syn-missing".into()),
        _ => Err("parser-pin:locked-syn-ambiguous".into()),
    }
}

fn string_array(value: Option<&toml::Value>) -> Result<Vec<&str>, String> {
    value
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "parser-pin:features".to_string())?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "parser-pin:feature-type".to_string())
        })
        .collect()
}

fn opaque_bytes(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let candidate = root.join(relative);
    let path = candidate
        .canonicalize()
        .map_err(|error| format!("missing-source:{relative}:{error}"))?;
    if !path.starts_with(root) || path != candidate {
        return Err(format!("unsafe-source-path:{relative}"));
    }
    fs::read(&path).map_err(|error| format!("read-source:{relative}:{error}"))
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|byte| {
            [
                DIGITS[(byte >> HEX_DIGIT_SHIFT) as usize] as char,
                DIGITS[(byte & HEX_DIGIT_MASK as u8) as usize] as char,
            ]
        })
        .collect()
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod ast_linter_source_tests;

#[cfg(test)]
#[path = "source_io_tests.rs"]
mod ast_linter_source_io_tests;
