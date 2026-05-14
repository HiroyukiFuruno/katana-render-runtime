use super::{RULE, RuntimeBundleRule, SOURCE};
use crate::diagnostics::{KdrLintError, Violation};
use std::path::Path;

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
        if path.contains("/source/shared/") {
            Self::reject_imports(
                file,
                source,
                ["../mermaid", "../drawio", "../zenuml"],
                violations,
            );
        }
        if path.contains("/source/mermaid/") {
            Self::reject_imports(file, source, ["../drawio", "../zenuml"], violations);
        }
        if path.contains("/source/drawio/") {
            Self::reject_imports(file, source, ["../mermaid", "../zenuml"], violations);
        }
        if path.contains("/source/zenuml/") {
            Self::reject_imports(file, source, ["../mermaid", "../drawio"], violations);
        }
    }

    fn reject_imports<const COUNT: usize>(
        file: &Path,
        source: &str,
        denied: [&str; COUNT],
        violations: &mut Vec<Violation>,
    ) {
        for (line_index, line) in source.lines().enumerate() {
            if denied.iter().any(|entry| line.contains(entry)) {
                violations.push(Violation::new(
                    file.to_path_buf(),
                    line_index + 1,
                    1,
                    RULE,
                    "runtime source directories must depend only on shared helpers.",
                ));
            }
        }
    }
}
