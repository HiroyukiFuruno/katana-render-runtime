use kdr_linter::{KdrLintError, KdrLinter, workspace::WorkspaceModel};
use std::path::{Path, PathBuf};

mod support;

use support::{TestResult, write_file};
#[cfg(unix)]
use support::{blocked_directory, set_mode};

#[test]
fn reports_rust_parse_error() -> TestResult<()> {
    let root = temp_root("parse");
    write_valid_manifests(&root)?;
    write_file(
        &root,
        "crates/katana-render-runtime/src/bad.rs",
        "fn broken(",
    )?;

    assert!(matches!(
        KdrLinter::lint_workspace(&root),
        Err(KdrLintError::RustParse { .. })
    ));
    Ok(())
}

#[test]
fn reports_toml_parse_error() -> TestResult<()> {
    let root = temp_root("toml");
    write_file(&root, "crates/katana-render-runtime/Cargo.toml", "[package")?;
    write_file(
        &root,
        "crates/katana-render-runtime-cli/Cargo.toml",
        cli_manifest(),
    )?;

    assert!(matches!(
        KdrLinter::lint_workspace(&root),
        Err(KdrLintError::TomlParse { .. })
    ));
    Ok(())
}

#[test]
fn reports_manifest_read_error() -> TestResult<()> {
    let root = temp_root("read");
    write_file(
        &root,
        "crates/katana-render-runtime/src/lib.rs",
        "struct Ok;",
    )?;

    assert!(matches!(
        KdrLinter::lint_workspace(&root),
        Err(KdrLintError::Read { .. })
    ));
    Ok(())
}

#[test]
#[cfg(unix)]
fn reports_source_read_error() -> TestResult<()> {
    let root = temp_root("source-read");
    write_valid_manifests(&root)?;
    let source = write_file(
        &root,
        "crates/katana-render-runtime/src/private.rs",
        "struct Ok;",
    )?;
    set_mode(&source, 0o000)?;

    let result = KdrLinter::lint_workspace(&root);
    set_mode(&source, 0o644)?;
    assert!(matches!(result, Err(KdrLintError::Read { .. })));
    Ok(())
}

#[test]
#[cfg(unix)]
fn reports_workspace_walk_error() -> TestResult<()> {
    let root = temp_root("walk");
    write_valid_manifests(&root)?;
    let blocked = blocked_directory(&root, "crates/katana-render-runtime/src/blocked")?;

    let result = KdrLinter::lint_workspace(&root);
    set_mode(&blocked, 0o755)?;
    assert!(matches!(result, Err(KdrLintError::Walk { .. })));
    Ok(())
}

#[test]
fn workspace_model_allows_missing_source_roots() -> TestResult<()> {
    let root = temp_root("empty");
    let workspace = WorkspaceModel::load(&root)?;

    assert!(workspace.rust_files().is_empty());
    assert_eq!(workspace.root(), root.as_path());
    Ok(())
}

#[test]
fn workspace_model_exposes_loaded_source() -> TestResult<()> {
    let root = temp_root("source-model");
    write_valid_manifests(&root)?;
    write_file(
        &root,
        "crates/katana-render-runtime/src/lib.rs",
        "pub struct Loaded;",
    )?;

    let workspace = WorkspaceModel::load(&root)?;
    let source_file = workspace
        .rust_files()
        .iter()
        .find(|it| it.path().ends_with("lib.rs"))
        .ok_or("expected loaded Rust source")?;

    assert_eq!(source_file.source(), "pub struct Loaded;");
    Ok(())
}

fn write_valid_manifests(root: &Path) -> TestResult<()> {
    support::write_manifest_pair(root, lib_manifest(), cli_manifest())
}

fn lib_manifest() -> &'static str {
    r#"
[package]
name = "katana-render-runtime"
version = "0.1.0"
edition = "2024"
"#
}

fn cli_manifest() -> &'static str {
    r#"
[package]
name = "katana-render-runtime-cli"
version = "0.1.0"
edition = "2024"

[dependencies]
katana-render-runtime = { path = "../katana-render-runtime" }
"#
}

fn temp_root(name: &str) -> PathBuf {
    support::temp_root("kdr-linter-error", name)
}
