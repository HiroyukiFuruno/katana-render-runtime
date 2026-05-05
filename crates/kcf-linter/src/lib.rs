#![deny(
    warnings,
    clippy::all,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::dbg_macro
)]

pub mod diagnostics;
pub mod rules;
pub mod span;
pub mod syntax;
pub mod workspace;

pub use diagnostics::{KcfLintError, Violation, ViolationReport};

use std::path::Path;

pub struct KcfLinter;

impl KcfLinter {
    pub fn lint_workspace(root: &Path) -> Result<Vec<Violation>, KcfLintError> {
        let workspace = workspace::WorkspaceModel::load(root)?;
        rules::RuleRunner::check(&workspace)
    }
}
