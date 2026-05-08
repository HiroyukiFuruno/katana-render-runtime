use kcf_linter::{KcfLintError, KcfLinter, workspace::WorkspaceModel};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn reports_rust_parse_error() -> TestResult<()> {
    let root = temp_root("parse");
    write_valid_manifests(&root)?;
    write_file(&root, "crates/katana-canvas-forge/src/bad.rs", "fn broken(")?;

    assert!(matches!(
        KcfLinter::lint_workspace(&root),
        Err(KcfLintError::RustParse { .. })
    ));
    Ok(())
}

#[test]
fn reports_toml_parse_error() -> TestResult<()> {
    let root = temp_root("toml");
    write_file(&root, "crates/katana-canvas-forge/Cargo.toml", "[package")?;
    write_file(
        &root,
        "crates/katana-canvas-forge-cli/Cargo.toml",
        cli_manifest(),
    )?;

    assert!(matches!(
        KcfLinter::lint_workspace(&root),
        Err(KcfLintError::TomlParse { .. })
    ));
    Ok(())
}

#[test]
fn reports_manifest_read_error() -> TestResult<()> {
    let root = temp_root("read");
    write_file(&root, "crates/katana-canvas-forge/src/lib.rs", "struct Ok;")?;

    assert!(matches!(
        KcfLinter::lint_workspace(&root),
        Err(KcfLintError::Read { .. })
    ));
    Ok(())
}

#[test]
fn reports_source_read_error() -> TestResult<()> {
    let root = temp_root("source-read");
    write_valid_manifests(&root)?;
    let source = write_file(
        &root,
        "crates/katana-canvas-forge/src/private.rs",
        "struct Ok;",
    )?;
    set_mode(&source, 0o000)?;

    let result = KcfLinter::lint_workspace(&root);
    set_mode(&source, 0o644)?;
    assert!(matches!(result, Err(KcfLintError::Read { .. })));
    Ok(())
}

#[test]
fn reports_workspace_walk_error() -> TestResult<()> {
    let root = temp_root("walk");
    write_valid_manifests(&root)?;
    let blocked = root.join("crates/katana-canvas-forge/src/blocked");
    std::fs::create_dir_all(&blocked)?;
    set_mode(&blocked, 0o000)?;

    let result = KcfLinter::lint_workspace(&root);
    set_mode(&blocked, 0o755)?;
    assert!(matches!(result, Err(KcfLintError::Walk { .. })));
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
        "crates/katana-canvas-forge/src/lib.rs",
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
    write_file(
        root,
        "crates/katana-canvas-forge/Cargo.toml",
        lib_manifest(),
    )?;
    write_file(
        root,
        "crates/katana-canvas-forge-cli/Cargo.toml",
        cli_manifest(),
    )?;
    Ok(())
}

fn lib_manifest() -> &'static str {
    r#"
[package]
name = "katana-canvas-forge"
version = "0.1.0"
edition = "2024"
"#
}

fn cli_manifest() -> &'static str {
    r#"
[package]
name = "katana-canvas-forge-cli"
version = "0.1.0"
edition = "2024"

[dependencies]
katana-canvas-forge = { path = "../katana-canvas-forge" }
"#
}

fn write_file(root: &Path, relative: &str, content: &str) -> TestResult<PathBuf> {
    let path = root.join(relative);
    let Some(parent) = path.parent() else {
        return Err(Box::new(std::io::Error::other("path has no parent")));
    };
    std::fs::create_dir_all(parent)?;
    std::fs::write(&path, content)?;
    Ok(path)
}

fn set_mode(path: &Path, mode: u32) -> TestResult<()> {
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(mode);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

fn temp_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kcf-linter-error-{name}-{}", std::process::id()))
}
