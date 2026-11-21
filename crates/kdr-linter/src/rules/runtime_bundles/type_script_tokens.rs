use super::{RULE, RuntimeBundleRule, SOURCE, TS_SCRIPT_ROOT};
use crate::diagnostics::{KdrLintError, Violation};
use std::path::Path;

const FORBIDDEN_TOKENS: &[&str] = &[
    " as any",
    ": any",
    "@ts-ignore",
    "@ts-expect-error",
    "biome-ignore",
];

const FORBIDDEN_RUNTIME_INTERNAL_TYPES: &[&str] = &["unknown", "Record<string, unknown>"];

pub(super) struct RuntimeTypeScriptTokenRule;

impl RuntimeTypeScriptTokenRule {
    pub(super) fn check(root: &Path, violations: &mut Vec<Violation>) -> Result<(), KdrLintError> {
        for file in RuntimeBundleRule::ts_files(root, SOURCE)? {
            let source = RuntimeBundleRule::read_source(&file)?;
            Self::check_forbidden_tokens(&file, &source, violations, true);
        }
        for file in RuntimeBundleRule::ts_files(root, TS_SCRIPT_ROOT)? {
            let source = RuntimeBundleRule::read_source(&file)?;
            Self::check_forbidden_tokens(&file, &source, violations, false);
        }
        Ok(())
    }

    fn check_forbidden_tokens(
        file: &Path,
        source: &str,
        violations: &mut Vec<Violation>,
        runtime_internal: bool,
    ) {
        let additional_tokens: &[&str] = if runtime_internal {
            FORBIDDEN_RUNTIME_INTERNAL_TYPES
        } else {
            &[]
        };
        for (line_index, line) in source.lines().enumerate() {
            for token in FORBIDDEN_TOKENS
                .iter()
                .chain(additional_tokens.iter())
                .copied()
            {
                if line.contains(token) {
                    push_token_violation(file, line_index + 1, token, runtime_internal, violations);
                }
            }
        }
    }
}

fn push_token_violation(
    file: &Path,
    line: usize,
    token: &str,
    runtime_internal: bool,
    violations: &mut Vec<Violation>,
) {
    let scope = if runtime_internal && FORBIDDEN_RUNTIME_INTERNAL_TYPES.contains(&token) {
        " in runtime internals"
    } else {
        ""
    };
    violations.push(Violation::new(
        file.to_path_buf(),
        line,
        1,
        RULE,
        format!("TypeScript runtime gate forbids `{token}`{scope}."),
    ));
}
