use kdr_linter::{KdrLinter, ViolationReport};
use std::path::{Path, PathBuf};

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn reports_imported_std_sync_rwlock() -> TestResult<()> {
    assert_prohibited_type("imported-rwlock", imported_rwlock_source())
}

#[test]
fn reports_renamed_std_sync_rwlock() -> TestResult<()> {
    assert_prohibited_type("renamed-rwlock", renamed_rwlock_source())
}

#[test]
fn reports_grouped_std_sync_rwlock() -> TestResult<()> {
    assert_prohibited_type("grouped-rwlock", grouped_rwlock_source())
}

#[test]
fn reports_glob_imported_std_sync_rwlock() -> TestResult<()> {
    assert_prohibited_type("glob-imported-rwlock", glob_imported_rwlock_source())
}

fn assert_prohibited_type(name: &str, source: &str) -> TestResult<()> {
    let root = temp_root(name);
    write_file(
        &root,
        "crates/katana-diagram-renderer/Cargo.toml",
        lib_manifest(),
    )?;
    write_file(
        &root,
        "crates/katana-diagram-renderer-cli/Cargo.toml",
        cli_manifest(),
    )?;
    write_file(&root, "crates/katana-diagram-renderer/src/lib.rs", source)?;

    let violations = KdrLinter::lint_workspace(&root)?;
    let report = ViolationReport::format(&violations);

    assert!(report.contains("prohibited-type"), "{report}");
    Ok(())
}

fn imported_rwlock_source() -> &'static str {
    r#"
use std::sync::RwLock;

type ImportedLock = RwLock<i32>;
"#
}

fn renamed_rwlock_source() -> &'static str {
    r#"
use std::sync::RwLock as StdRwLock;

type ImportedLock = StdRwLock<i32>;
"#
}

fn grouped_rwlock_source() -> &'static str {
    r#"
use std::sync::{Arc, RwLock};

type ImportedLock = RwLock<i32>;
"#
}

fn glob_imported_rwlock_source() -> &'static str {
    r#"
use std::sync::*;

type ImportedLock = RwLock<i32>;
"#
}

fn lib_manifest() -> &'static str {
    r#"
[package]
name = "katana-diagram-renderer"
version = "0.1.0"
edition = "2024"

[dependencies]
"#
}

fn cli_manifest() -> &'static str {
    r#"
[package]
name = "katana-diagram-renderer-cli"
version = "0.1.0"
edition = "2024"

[dependencies]
katana-diagram-renderer = { path = "../katana-diagram-renderer" }
"#
}

fn write_file(root: &Path, relative: &str, content: &str) -> TestResult<()> {
    let path = root.join(relative);
    let Some(parent) = path.parent() else {
        return Err(Box::new(std::io::Error::other("path has no parent")));
    };
    std::fs::create_dir_all(parent)?;
    std::fs::write(path, content)?;
    Ok(())
}

fn temp_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kdr-linter-{name}-{}", std::process::id()))
}
