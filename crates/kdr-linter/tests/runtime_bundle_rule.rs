#[cfg(unix)]
use kdr_linter::KdrLintError;
use kdr_linter::{KdrLinter, ViolationReport};
use std::path::{Path, PathBuf};

mod support;

use support::{TestResult, write_file};
#[cfg(unix)]
use support::{blocked_directory, set_mode};

#[test]
fn accepts_complete_runtime_bundle_layout() -> TestResult<()> {
    let root = temp_root("complete");
    write_clean_fixture(&root)?;

    let report = runtime_report(&root)?;

    assert!(!report.contains("[runtime-bundle-boundary]"), "{report}");
    Ok(())
}

#[test]
fn reports_runtime_bundle_boundary_violations() -> TestResult<()> {
    let root = temp_root("violations");
    write_clean_fixture(&root)?;
    write_violation_fixture(&root)?;

    let report = runtime_report(&root)?;

    for message in required_messages() {
        assert!(report.contains(message), "{report}");
    }
    Ok(())
}

#[test]
fn allows_unknown_types_at_node_validation_boundaries() -> TestResult<()> {
    let root = temp_root("unknown-validation-boundary");
    write_clean_fixture(&root)?;
    write_unknown_validation_boundary(&root)?;

    let report = runtime_report(&root)?;

    assert!(
        !report.contains("TypeScript runtime gate forbids `unknown`"),
        "{report}"
    );
    assert!(
        !report.contains("TypeScript runtime gate forbids `Record<string, unknown>`"),
        "{report}"
    );
    Ok(())
}

fn write_unknown_validation_boundary(root: &Path) -> TestResult<()> {
    write_file(
        root,
        "scripts/html/input-boundary.ts",
        concat!(
            "function record(value: unknown): Record<string, unknown> {\n",
            "  if (!value || typeof value !== \"object\" || Array.isArray(value)) throw new Error(\"object required\");\n",
            "  return value as Record<string, unknown>;\n",
            "}\n",
            "function strings(value: unknown): string[] {\n",
            "  if (!Array.isArray(value) || !value.every((item) => typeof item === \"string\")) throw new Error(\"strings required\");\n",
            "  return value as string[];\n",
            "}\n"
        ),
    )?;
    Ok(())
}

fn write_violation_fixture(root: &Path) -> TestResult<()> {
    write_file(
        root,
        "crates/katana-render-runtime/src/runtime.rs",
        concat!(
            "const SOURCE: &str = include",
            "_str!(\"js_runtime/runtime.js\");\n"
        ),
    )?;
    write_file(
        root,
        "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime_scripts.rs",
        "fn install_adapter() { katanaInstallMermaidZenumlRuntimeAdapter(); }\n",
    )?;
    write_file(
        root,
        "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/mermaid-runtime.min.js",
        "globalThis.notTheMermaidEntrypoint = () => {};\n",
    )?;
    write_unsafe_type_script(root)?;
    Ok(())
}

fn write_unsafe_type_script(root: &Path) -> TestResult<()> {
    write_file(
        root,
        "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mermaid/bad.ts",
        "import value from \"../shared/value\";\nimport drawio from \"#drawio/runtime\";\nimport legacy from \"@shared/runtime\";\nexport blocked from '#drawio/runtime';\nexport value from package;\nconst unsafeValue: unknown = value as any;\n",
    )?;
    write_file(
        root,
        "scripts/runtime-bundles/unsafe.ts",
        concat!(
            "const unsafe: any = value as any;\n",
            "// @ts-ignore\n",
            "// @ts-expect-error\n",
            "// biome-ignore lint/suspicious/noExplicitAny: fixture\n"
        ),
    )?;
    Ok(())
}

#[test]
fn reports_missing_runtime_bundle_paths() -> TestResult<()> {
    let root = temp_root("missing");
    write_manifests(&root)?;

    let report = runtime_report(&root)?;

    assert!(
        report.contains("required runtime bundle path is missing"),
        "{report}"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn reports_runtime_source_read_error() -> TestResult<()> {
    let root = temp_root("source-read");
    write_clean_fixture(&root)?;
    let source = root
        .join("crates/katana-render-runtime/src/markdown/diagram_runtime/source/mermaid/index.ts");
    set_mode(&source, 0o000)?;

    let result = KdrLinter::lint_workspace(&root);

    set_mode(&source, 0o644)?;
    assert!(matches!(result, Err(KdrLintError::Read { .. })));
    Ok(())
}

#[test]
#[cfg(unix)]
fn reports_runtime_script_read_error() -> TestResult<()> {
    let root = temp_root("script-read");
    write_clean_fixture(&root)?;
    let script = root.join("scripts/runtime-bundles/bundle-runtime.ts");
    set_mode(&script, 0o000)?;

    let result = KdrLinter::lint_workspace(&root);

    set_mode(&script, 0o644)?;
    assert!(matches!(result, Err(KdrLintError::Read { .. })));
    Ok(())
}

#[test]
#[cfg(unix)]
fn reports_runtime_source_walk_error() -> TestResult<()> {
    let root = temp_root("source-walk");
    write_clean_fixture(&root)?;
    let blocked = blocked_directory(
        &root,
        "crates/katana-render-runtime/src/markdown/diagram_runtime/source/blocked",
    )?;

    let result = KdrLinter::lint_workspace(&root);

    set_mode(&blocked, 0o755)?;
    assert!(matches!(result, Err(KdrLintError::Walk { .. })));
    Ok(())
}

#[test]
#[cfg(unix)]
fn reports_runtime_script_walk_error() -> TestResult<()> {
    let root = temp_root("script-walk");
    write_clean_fixture(&root)?;
    let blocked = blocked_directory(&root, "scripts/runtime-bundles/blocked")?;

    let result = KdrLinter::lint_workspace(&root);

    set_mode(&blocked, 0o755)?;
    assert!(matches!(result, Err(KdrLintError::Walk { .. })));
    Ok(())
}

fn required_messages() -> [&'static str; 10] {
    [
        "V8 runtime code must be included from generated bundles",
        "Mermaid render script must not call the ZenUML adapter installer directly",
        "generated runtime bundle must publish `katanaRunMermaidRuntime` via globalThis",
        "runtime source imports must use package imports for approved boundaries",
        "TypeScript runtime gate forbids `unknown`",
        "TypeScript runtime gate forbids ` as any`",
        "TypeScript runtime gate forbids `: any`",
        "TypeScript runtime gate forbids `@ts-ignore`",
        "TypeScript runtime gate forbids `@ts-expect-error`",
        "TypeScript runtime gate forbids `biome-ignore`",
    ]
}

fn runtime_report(root: &Path) -> TestResult<String> {
    let violations = KdrLinter::lint_workspace(root)?;
    Ok(ViolationReport::format(&violations))
}

fn write_clean_fixture(root: &Path) -> TestResult<()> {
    write_manifests(root)?;
    write_rust_files(root)?;
    write_generated_bundles(root)?;
    write_runtime_sources(root)?;
    Ok(())
}

fn write_rust_files(root: &Path) -> TestResult<()> {
    write_file(
        root,
        "crates/katana-render-runtime/src/lib.rs",
        "pub(crate) struct Runtime;\n",
    )?;
    write_file(
        root,
        "crates/katana-render-runtime-cli/src/lib.rs",
        "pub(crate) struct Command;\n",
    )?;
    write_file(
        root,
        "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime_scripts.rs",
        "const RUNTIME: &str = include_str!(\"../diagram_runtime/generated/mermaid-runtime.min.js\");\n",
    )?;
    Ok(())
}

fn write_generated_bundles(root: &Path) -> TestResult<()> {
    write_bundle(root, "mermaid", "katanaRunMermaidRuntime")?;
    write_bundle(root, "drawio", "katanaRunDrawioRuntime")?;
    write_bundle(root, "zenuml", "katanaRunZenumlRuntime")?;
    write_file(
        root,
        "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/mathjax-runtime.min.js",
        "globalThis.katanaRunMathJaxRuntime = () => {};\n",
    )?;
    write_file(
        root,
        "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/runtime-bundles.sha256",
        "fixture\n",
    )?;
    write_file(
        root,
        "scripts/runtime-bundles/bundle-runtime.ts",
        "export {};\n",
    )?;
    Ok(())
}

fn write_runtime_sources(root: &Path) -> TestResult<()> {
    for runtime in ["shared", "mermaid", "drawio", "zenuml", "mathjax"] {
        write_file(
            root,
            &format!(
                "crates/katana-render-runtime/src/markdown/diagram_runtime/source/{runtime}/index.ts"
            ),
            "export {};\n",
        )?;
    }
    Ok(())
}

fn write_bundle(root: &Path, bundle: &str, entrypoint: &str) -> TestResult<()> {
    write_file(
        root,
        &format!(
            "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/{bundle}-runtime.min.js"
        ),
        &format!("globalThis[\"{entrypoint}\"] = () => {{}};\n"),
    )?;
    Ok(())
}

fn write_manifests(root: &Path) -> TestResult<()> {
    support::write_manifest_pair(
        root,
        "[package]\nname = \"katana-render-runtime\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        "[package]\nname = \"katana-render-runtime-cli\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nkatana-render-runtime = { path = \"../katana-render-runtime\" }\n",
    )
}

fn temp_root(name: &str) -> PathBuf {
    support::temp_root("kdr-linter-runtime-bundle", name)
}
