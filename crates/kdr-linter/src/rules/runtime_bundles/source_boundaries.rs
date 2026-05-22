use super::{RULE, RuntimeBundleRule, SOURCE};
use crate::diagnostics::{KdrLintError, Violation};
use std::path::Path;

const SHARED_DENIED: &[&str] = &["../mermaid", "../drawio", "../zenuml", "../mathjax"];
const MERMAID_DENIED: &[&str] = &["../drawio", "../zenuml", "../mathjax"];
const DRAWIO_DENIED: &[&str] = &["../mermaid", "../zenuml", "../mathjax"];
const ZENUML_DENIED: &[&str] = &["../mermaid", "../drawio", "../mathjax"];
const MATHJAX_DENIED: &[&str] = &["../mermaid", "../drawio", "../zenuml"];

const SOURCE_BOUNDARIES: &[RuntimeSourceBoundary] = &[
    RuntimeSourceBoundary::new("/source/shared/", SHARED_DENIED),
    RuntimeSourceBoundary::new("/source/mermaid/", MERMAID_DENIED),
    RuntimeSourceBoundary::new("/source/drawio/", DRAWIO_DENIED),
    RuntimeSourceBoundary::new("/source/zenuml/", ZENUML_DENIED),
    RuntimeSourceBoundary::new("/source/mathjax/", MATHJAX_DENIED),
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
        for boundary in SOURCE_BOUNDARIES {
            if path.contains(boundary.marker) {
                Self::reject_imports(file, source, boundary.denied, violations);
            }
        }
    }

    fn reject_imports(file: &Path, source: &str, denied: &[&str], violations: &mut Vec<Violation>) {
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

struct RuntimeSourceBoundary {
    marker: &'static str,
    denied: &'static [&'static str],
}

impl RuntimeSourceBoundary {
    const fn new(marker: &'static str, denied: &'static [&'static str]) -> Self {
        Self { marker, denied }
    }
}
