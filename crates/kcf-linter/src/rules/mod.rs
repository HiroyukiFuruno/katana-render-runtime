mod architecture;
mod attributes;
mod cli_duplication;
mod function_length;
mod lazy_code;
mod manifest_boundary;
mod method_calls;
mod prohibited_types;

use crate::diagnostics::{KcfLintError, Violation};
use crate::workspace::WorkspaceModel;
use architecture::ArchitectureRule;
use attributes::ProhibitedAttributeRule;
use function_length::FunctionLengthRule;
use lazy_code::LazyCodeRule;
use method_calls::ProhibitedMethodRule;
use prohibited_types::ProhibitedTypeRule;

type RuleCheck = fn(&WorkspaceModel) -> Result<Vec<Violation>, KcfLintError>;
const SUPPLEMENTAL_RULE_COUNT: usize = 6;

pub struct RuleRunner;

impl RuleRunner {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KcfLintError> {
        let mut violations = Vec::new();
        for rule in Self::rules() {
            violations.extend(rule(workspace)?);
        }
        Ok(violations)
    }

    fn rules() -> [RuleCheck; SUPPLEMENTAL_RULE_COUNT] {
        [
            FunctionLengthRule::check,
            ProhibitedMethodRule::check,
            ProhibitedTypeRule::check,
            LazyCodeRule::check,
            ProhibitedAttributeRule::check,
            ArchitectureRule::check,
        ]
    }
}
