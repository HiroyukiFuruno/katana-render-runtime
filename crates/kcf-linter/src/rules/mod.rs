mod architecture;
mod attributes;
mod cli_duplication;
mod error_first;
mod file_length;
mod function_length;
mod lazy_code;
mod manifest_boundary;
mod method_calls;
mod nesting_depth;
mod prohibited_types;
mod pub_free_fn;
mod type_separation;

use crate::diagnostics::{KcfLintError, Violation};
use crate::workspace::WorkspaceModel;
use architecture::ArchitectureRule;
use attributes::ProhibitedAttributeRule;
use error_first::ErrorFirstRule;
use file_length::FileLengthRule;
use function_length::FunctionLengthRule;
use lazy_code::LazyCodeRule;
use method_calls::ProhibitedMethodRule;
use nesting_depth::NestingDepthRule;
use prohibited_types::ProhibitedTypeRule;
use pub_free_fn::PublicFreeFunctionRule;
use type_separation::TypeSeparationRule;

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

    fn rules() -> [RuleCheck; 11] {
        [
            FileLengthRule::check,
            FunctionLengthRule::check,
            NestingDepthRule::check,
            ErrorFirstRule::check,
            TypeSeparationRule::check,
            PublicFreeFunctionRule::check,
            ProhibitedMethodRule::check,
            ProhibitedTypeRule::check,
            LazyCodeRule::check,
            ProhibitedAttributeRule::check,
            ArchitectureRule::check,
        ]
    }
}
