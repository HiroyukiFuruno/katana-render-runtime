use kcf_linter::{KcfLinter, ViolationReport};
use std::path::{Path, PathBuf};

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn reports_all_structural_rule_families() -> TestResult<()> {
    let root = temp_root("rules");
    write_manifests(&root)?;
    write_file(
        &root,
        "crates/katana-canvas-forge/src/bad.rs",
        &bad_lib_source(),
    )?;
    write_file(
        &root,
        "crates/katana-canvas-forge-cli/src/bad.rs",
        cli_duplicate_source(),
    )?;

    let violations = KcfLinter::lint_workspace(&root)?;
    let report = ViolationReport::format(&violations);
    for rule in required_rules() {
        assert!(report.contains(rule), "{report}");
    }
    Ok(())
}

fn required_rules() -> [&'static str; 8] {
    [
        "file-length",
        "function-length",
        "nesting-depth",
        "public-free-function",
        "prohibited-method",
        "lazy-code",
        "prohibited-attribute",
        "renderer-boundary",
    ]
}

fn bad_lib_source() -> String {
    let mut source = String::from(bad_lib_header());
    append_oversized_function(&mut source);
    append_file_length_filler(&mut source);
    source
}

fn bad_lib_header() -> &'static str {
    r#"
#[allow(dead_code)]
pub fn exposed() {
    let value = Some(1);
    let _ = value.unwrap();
    todo!();
    unimplemented!();
    dbg!(value);
    if true { if true { if true { if true {} } } }
}

pub(crate) fn crate_visible() {}

#[cfg(test)]
pub fn test_only_allowed() {}

#[cfg(test)]
mod tests {
    pub fn nested_test_allowed() {}
}

fn main() {}

struct Long;
impl Long {
    fn oversized() {
"#
}

fn append_oversized_function(source: &mut String) {
    for line in 0..35 {
        source.push_str(&format!("        let _line_{line} = {line};\n"));
    }
    source.push_str("    }\n}\n");
}

fn append_file_length_filler(source: &mut String) {
    for line in 0..180 {
        source.push_str(&format!("// filler {line}\n"));
    }
}

fn cli_duplicate_source() -> &'static str {
    r#"
struct RenderInput;
enum RenderError {}
trait Renderer {}
mod renderer {}
"#
}

fn write_manifests(root: &Path) -> TestResult<()> {
    write_file(
        root,
        "crates/katana-canvas-forge/Cargo.toml",
        lib_manifest(),
    )?;
    write_file(
        root,
        "crates/katana-canvas-forge-cli/Cargo.toml",
        cli_manifest_missing_lib(),
    )
}

fn lib_manifest() -> &'static str {
    r#"
[package]
name = "katana-canvas-forge"
version = "0.1.0"
edition = "2024"

[dependencies]
egui = "0.1"
cli_alias = { package = "katana-canvas-forge-cli", version = "0.1" }
"#
}

fn cli_manifest_missing_lib() -> &'static str {
    r#"
[package]
name = "katana-canvas-forge-cli"
version = "0.1.0"
edition = "2024"

[dependencies]
clap = "4"
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
    std::env::temp_dir().join(format!("kcf-linter-{name}-{}", std::process::id()))
}
