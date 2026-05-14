use super::{RULE, RuntimeBundleRule, SOURCE, TS_SCRIPT_ROOT};
use crate::diagnostics::{KcfLintError, Violation};
use std::path::Path;

const FORBIDDEN_TOKENS: &[&str] = &[
    "unknown",
    "Record<string, unknown>",
    " as any",
    ": any",
    "@ts-ignore",
    "@ts-expect-error",
    "biome-ignore",
];

pub(super) struct RuntimeTypeScriptTokenRule;

impl RuntimeTypeScriptTokenRule {
    pub(super) fn check(root: &Path, violations: &mut Vec<Violation>) -> Result<(), KcfLintError> {
        for scan_root in [SOURCE, TS_SCRIPT_ROOT] {
            for file in RuntimeBundleRule::ts_files(root, scan_root)? {
                let source = RuntimeBundleRule::read_source(&file)?;
                Self::check_forbidden_tokens(&file, &source, violations);
            }
        }
        Ok(())
    }

    fn check_forbidden_tokens(file: &Path, source: &str, violations: &mut Vec<Violation>) {
        for (line_index, line) in source.lines().enumerate() {
            for token in FORBIDDEN_TOKENS.iter().copied() {
                if line.contains(token) {
                    violations.push(Violation::new(
                        file.to_path_buf(),
                        line_index + 1,
                        1,
                        RULE,
                        format!("TypeScript runtime gate forbids `{token}`."),
                    ));
                }
            }
        }
    }
}
