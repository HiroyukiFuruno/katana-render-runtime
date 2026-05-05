use crate::diagnostics::{KcfLintError, Violation};
use crate::workspace::WorkspaceModel;

const MAX_FILE_LINES: usize = 200;

pub struct FileLengthRule;

impl FileLengthRule {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KcfLintError> {
        let mut violations = Vec::new();
        for file in workspace.rust_files() {
            let lines = file.source().lines().count();
            if lines > MAX_FILE_LINES {
                violations.push(Violation::new(
                    file.path().to_path_buf(),
                    1,
                    1,
                    "file-length",
                    format!(
                        "file has {lines} lines. Split by responsibility and keep each file <= {MAX_FILE_LINES} lines."
                    ),
                ));
            }
        }
        Ok(violations)
    }
}
