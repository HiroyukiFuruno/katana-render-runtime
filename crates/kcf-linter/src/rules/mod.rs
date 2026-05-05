mod architecture;
mod attributes;
mod cli_duplication;
mod file_length;
mod function_length;
mod lazy_code;
mod manifest_boundary;
mod method_calls;
mod nesting_depth;
mod pub_free_fn;

use crate::diagnostics::{KcfLintError, Violation};
use crate::workspace::WorkspaceModel;
use architecture::ArchitectureRule;
use attributes::ProhibitedAttributeRule;
use file_length::FileLengthRule;
use function_length::FunctionLengthRule;
use lazy_code::LazyCodeRule;
use method_calls::ProhibitedMethodRule;
use nesting_depth::NestingDepthRule;
use pub_free_fn::PublicFreeFunctionRule;

type RuleCheck = fn(&WorkspaceModel) -> Result<Vec<Violation>, KcfLintError>;

pub struct RuleRunner;

impl RuleRunner {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KcfLintError> {
        let mut violations = Vec::new();
        for rule in Self::rules() {
            violations.extend(rule(workspace)?);
        }
        Ok(violations)
    }

    fn rules() -> [RuleCheck; 8] {
        [
            FileLengthRule::check,
            FunctionLengthRule::check,
            NestingDepthRule::check,
            PublicFreeFunctionRule::check,
            ProhibitedMethodRule::check,
            LazyCodeRule::check,
            ProhibitedAttributeRule::check,
            ArchitectureRule::check,
        ]
    }
}
