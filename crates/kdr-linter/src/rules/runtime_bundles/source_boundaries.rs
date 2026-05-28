use super::{RULE, RuntimeBundleRule, SOURCE};
use crate::diagnostics::{KdrLintError, Violation};
use std::path::Path;

const FORBIDDEN_IMPORT_PREFIXES: &[&str] = &[
    "@shared/",
    "@mermaid/",
    "@drawio/",
    "@zenuml/",
    "#/shared/",
    "#/mermaid/",
    "#/drawio/",
    "#/zenuml/",
];

const SOURCE_BOUNDARIES: &[RuntimeSourceBoundary] = &[
    RuntimeSourceBoundary::new("/source/shared/", &["#shared/"]),
    RuntimeSourceBoundary::new("/source/mermaid/", &["#shared/", "#mermaid/"]),
    RuntimeSourceBoundary::new("/source/drawio/", &["#shared/", "#drawio/"]),
    RuntimeSourceBoundary::new("/source/zenuml/", &["#shared/", "#zenuml/"]),
    RuntimeSourceBoundary::new("/source/mathjax/", &["#shared/"]),
];

pub(super) struct RuntimeSourceBoundaryRule;

impl RuntimeSourceBoundaryRule {
    pub(super) fn check(root: &Path, violations: &mut Vec<Violation>) -> Result<(), KdrLintError> {
        for file in RuntimeBundleRule::ts_files(root, SOURCE)? {
            let source = RuntimeBundleRule::read_source(&file)?;
            Self::check_import_boundary(&file, &source, violations);
        }
        Ok(())
    }

    fn check_import_boundary(file: &Path, source: &str, violations: &mut Vec<Violation>) {
        let path = file.to_string_lossy();
        let boundary = SOURCE_BOUNDARIES
            .iter()
            .find(|candidate| path.contains(candidate.marker));
        for (line_index, line) in source.lines().enumerate() {
            if !Self::is_import_line(line) {
                continue;
            }
            Self::reject_forbidden_prefixes(file, line_index, line, violations);
            if let Some(rule) = boundary {
                Self::reject_boundary_crossing_import(file, line_index, line, rule, violations);
            }
        }
    }

    fn is_import_line(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.starts_with("import ")
            || trimmed.starts_with("export ")
            || trimmed.contains(" from \"")
            || trimmed.contains(" from '")
    }

    fn reject_forbidden_prefixes(
        file: &Path,
        line_index: usize,
        line: &str,
        violations: &mut Vec<Violation>,
    ) {
        if FORBIDDEN_IMPORT_PREFIXES
            .iter()
            .any(|prefix| line.contains(prefix))
        {
            violations.push(Self::violation(file, line_index));
        }
    }

    fn reject_boundary_crossing_import(
        file: &Path,
        line_index: usize,
        line: &str,
        boundary: &RuntimeSourceBoundary,
        violations: &mut Vec<Violation>,
    ) {
        if line.contains("\"../") || line.contains("'../") {
            violations.push(Self::violation(file, line_index));
            return;
        }
        let Some(specifier) = ImportSpecifier::parse(line) else {
            return;
        };
        if specifier.starts_with('#')
            && !boundary
                .allowed_package_imports
                .iter()
                .any(|prefix| specifier.starts_with(prefix))
        {
            violations.push(Self::violation(file, line_index));
        }
    }

    fn violation(file: &Path, line_index: usize) -> Violation {
        Violation::new(
            file.to_path_buf(),
            line_index + 1,
            1,
            RULE,
            "runtime source imports must use package imports for approved boundaries.",
        )
    }
}

struct RuntimeSourceBoundary {
    marker: &'static str,
    allowed_package_imports: &'static [&'static str],
}

impl RuntimeSourceBoundary {
    const fn new(marker: &'static str, allowed_package_imports: &'static [&'static str]) -> Self {
        Self {
            marker,
            allowed_package_imports,
        }
    }
}

struct ImportSpecifier;

impl ImportSpecifier {
    fn parse(line: &str) -> Option<&str> {
        let quote = Self::quote(line)?;
        let start = line.find(quote)? + quote.len_utf8();
        let end = start + line[start..].find(quote)?;
        Some(&line[start..end])
    }

    fn quote(line: &str) -> Option<char> {
        if line.contains('"') {
            return Some('"');
        }
        if line.contains('\'') {
            return Some('\'');
        }
        None
    }
}
