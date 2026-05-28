mod source_boundaries;
mod type_script_tokens;

use crate::diagnostics::{KdrLintError, Violation};
use crate::workspace::WorkspaceModel;
use ignore::WalkBuilder;
use source_boundaries::RuntimeSourceBoundaryRule;
use std::path::{Path, PathBuf};
use type_script_tokens::RuntimeTypeScriptTokenRule;

const RULE: &str = "runtime-bundle-boundary";
const RUNTIME_ROOT: &str = "crates/katana-render-runtime/src/markdown/diagram_runtime";
const GENERATED: &str = "crates/katana-render-runtime/src/markdown/diagram_runtime/generated";
const SOURCE: &str = "crates/katana-render-runtime/src/markdown/diagram_runtime/source";
const TS_SCRIPT_ROOT: &str = "scripts";
const REQUIRED_PATHS: &[&str] = &[
    RUNTIME_ROOT,
    GENERATED,
    SOURCE,
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/shared",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mermaid",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/drawio",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/zenuml",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mathjax",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/mermaid-runtime.min.js",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/drawio-runtime.min.js",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/zenuml-runtime.min.js",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/mathjax-runtime.min.js",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/runtime-bundles.sha256",
    "scripts/runtime-bundles/bundle-runtime.ts",
];
const MERMAID_RENDER_SCRIPT: &str =
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime_scripts.rs";
const GENERATED_ENTRYPOINTS: &[(&str, &str)] = &[
    (
        "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/mermaid-runtime.min.js",
        "katanaRunMermaidRuntime",
    ),
    (
        "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/drawio-runtime.min.js",
        "katanaRunDrawioRuntime",
    ),
    (
        "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/zenuml-runtime.min.js",
        "katanaRunZenumlRuntime",
    ),
];

pub(super) struct RuntimeBundleRule;

impl RuntimeBundleRule {
    pub(super) fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KdrLintError> {
        let mut violations = Vec::new();
        let root = workspace.root();
        Self::check_paths(root, &mut violations);
        Self::check_rust_includes(workspace, &mut violations);
        Self::check_runtime_entrypoints(root, &mut violations);
        RuntimeSourceBoundaryRule::check(root, &mut violations)?;
        RuntimeTypeScriptTokenRule::check(root, &mut violations)?;
        Ok(violations)
    }

    fn check_paths(root: &Path, violations: &mut Vec<Violation>) {
        for relative in REQUIRED_PATHS.iter().copied() {
            let path = root.join(relative);
            if !path.exists() {
                violations.push(Self::violation(
                    path,
                    "required runtime bundle path is missing",
                ));
            }
        }
    }

    fn check_rust_includes(workspace: &WorkspaceModel, violations: &mut Vec<Violation>) {
        for file in workspace.rust_files() {
            for (line_index, line) in file.source().lines().enumerate() {
                if line.contains("include_str!(") {
                    Self::check_include_line(file.path(), line_index, line, violations);
                }
            }
        }
    }

    fn check_include_line(
        file: &Path,
        line_index: usize,
        line: &str,
        violations: &mut Vec<Violation>,
    ) {
        if line.contains("diagram_runtime/generated") || line.contains("vendor/") {
            return;
        }
        if line.contains("js_runtime/") || line.contains("diagram_runtime/source") {
            violations.push(Violation::new(
                file.to_path_buf(),
                line_index + 1,
                1,
                RULE,
                "V8 runtime code must be included from generated bundles, not source fragments.",
            ));
        }
    }

    fn check_runtime_entrypoints(root: &Path, violations: &mut Vec<Violation>) {
        Self::check_mermaid_render_script(root, violations);
        for (relative_path, entrypoint) in GENERATED_ENTRYPOINTS {
            let path = root.join(relative_path);
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            let quoted = format!("globalThis[\"{entrypoint}\"]");
            let dotted = format!("globalThis.{entrypoint}");
            if !source.contains(&quoted) && !source.contains(&dotted) {
                violations.push(Violation::new(
                    path,
                    1,
                    1,
                    RULE,
                    format!("generated runtime bundle must publish `{entrypoint}` via globalThis."),
                ));
            }
        }
    }

    fn check_mermaid_render_script(root: &Path, violations: &mut Vec<Violation>) {
        let path = root.join(MERMAID_RENDER_SCRIPT);
        let Ok(source) = std::fs::read_to_string(&path) else {
            return;
        };
        for (line_index, line) in source.lines().enumerate() {
            if line.contains("katanaInstallMermaidZenumlRuntimeAdapter()") {
                violations.push(Violation::new(
                    path.clone(),
                    line_index + 1,
                    1,
                    RULE,
                    "Mermaid render script must not call the ZenUML adapter installer directly.",
                ));
            }
        }
    }

    pub(super) fn ts_files(root: &Path, relative: &str) -> Result<Vec<PathBuf>, KdrLintError> {
        let target = root.join(relative);
        let mut files = Vec::new();
        if !target.exists() {
            return Ok(files);
        }
        for entry in WalkBuilder::new(&target).standard_filters(true).build() {
            let entry = entry.map_err(|source| KdrLintError::Walk {
                path: target.clone(),
                source,
            })?;
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|extension| extension == "ts") {
                files.push(path.to_path_buf());
            }
        }
        files.sort();
        Ok(files)
    }

    pub(super) fn read_source(path: &Path) -> Result<String, KdrLintError> {
        std::fs::read_to_string(path).map_err(|source| KdrLintError::Read {
            path: path.to_path_buf(),
            source,
        })
    }

    fn violation(path: PathBuf, message: &'static str) -> Violation {
        Violation::new(path, 1, 1, RULE, message)
    }
}
