use crate::diagnostics::{KdrLintError, Violation};
use crate::workspace::WorkspaceModel;

use super::cli_duplication::CliDuplicationRule;
use super::manifest_boundary::ManifestBoundaryRule;

pub const LIB_CRATE: &str = "crates/katana-diagram-renderer";
pub const CLI_CRATE: &str = "crates/katana-diagram-renderer-cli";

pub struct ArchitectureRule;

impl ArchitectureRule {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KdrLintError> {
        let mut violations = Vec::new();
        violations.extend(ManifestBoundaryRule::check(workspace.root())?);
        violations.extend(CliDuplicationRule::check(workspace));
        Ok(violations)
    }
}
