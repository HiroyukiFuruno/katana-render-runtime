use crate::diagnostics::{KdrLintError, Violation};
use std::path::Path;

use super::architecture::{CLI_CRATE, LIB_CRATE};

pub(super) struct ManifestBoundaryRule;

impl ManifestBoundaryRule {
    pub(super) fn check(root: &Path) -> Result<Vec<Violation>, KdrLintError> {
        let mut violations = Vec::new();
        let lib_manifest = root.join(LIB_CRATE).join("Cargo.toml");
        let cli_manifest = root.join(CLI_CRATE).join("Cargo.toml");
        Self::check_renderer_manifest(&lib_manifest, &mut violations)?;
        Self::check_cli_manifest(&cli_manifest, &mut violations)?;
        Ok(violations)
    }

    fn check_renderer_manifest(
        path: &Path,
        violations: &mut Vec<Violation>,
    ) -> Result<(), KdrLintError> {
        let manifest = ManifestReader::read(path)?;
        for dependency in ManifestReader::dependency_names(&manifest) {
            if !Self::is_renderer_boundary_violation(&dependency) {
                continue;
            }
            violations.push(Self::manifest_violation(path, dependency));
        }
        Ok(())
    }

    fn check_cli_manifest(
        path: &Path,
        violations: &mut Vec<Violation>,
    ) -> Result<(), KdrLintError> {
        let manifest = ManifestReader::read(path)?;
        let dependencies = ManifestReader::dependency_names(&manifest);
        if dependencies
            .iter()
            .any(|it| it == "katana-diagram-renderer")
        {
            return Ok(());
        }
        violations.push(Violation::new(
            path.to_path_buf(),
            1,
            1,
            "cli-library-boundary",
            "CLI crate must depend on the library API instead of owning renderer runtime logic.",
        ));
        Ok(())
    }

    fn is_renderer_boundary_violation(dependency: &str) -> bool {
        UiDependencyPolicy::is_ui_dependency(dependency)
            || dependency == "katana-diagram-renderer-cli"
    }

    fn manifest_violation(path: &Path, dependency: String) -> Violation {
        Violation::new(
            path.to_path_buf(),
            1,
            1,
            "renderer-boundary",
            format!("renderer crate must not depend on UI or CLI crate `{dependency}`."),
        )
    }
}

struct ManifestReader;

impl ManifestReader {
    fn read(path: &Path) -> Result<toml::Value, KdrLintError> {
        let source = std::fs::read_to_string(path).map_err(|source| KdrLintError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&source).map_err(|source| KdrLintError::TomlParse {
            path: path.to_path_buf(),
            source,
        })
    }

    fn dependency_names(manifest: &toml::Value) -> Vec<String> {
        let mut names = Vec::new();
        for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
            Self::push_dependency_table(manifest, table, &mut names);
        }
        names
    }

    fn push_dependency_table(manifest: &toml::Value, table: &str, names: &mut Vec<String>) {
        let Some(dependencies) = manifest.get(table).and_then(toml::Value::as_table) else {
            return;
        };
        for (key, value) in dependencies {
            names.push(key.to_string());
            if let Some(package) = value.get("package").and_then(toml::Value::as_str) {
                names.push(package.to_string());
            }
        }
    }
}

struct UiDependencyPolicy;

impl UiDependencyPolicy {
    fn is_ui_dependency(name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        lower.ends_with("-ui")
            || lower.ends_with("_ui")
            || matches!(
                lower.as_str(),
                "dioxus" | "eframe" | "egui" | "iced" | "leptos" | "tauri" | "yew"
            )
    }
}
