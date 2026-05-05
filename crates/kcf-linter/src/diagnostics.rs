use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Violation {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub rule: &'static str,
    pub message: String,
}

impl Violation {
    pub fn new(
        file: PathBuf,
        line: usize,
        column: usize,
        rule: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            file,
            line,
            column,
            rule,
            message: message.into(),
        }
    }
}

pub struct ViolationReport;

impl ViolationReport {
    pub fn format(violations: &[Violation]) -> String {
        let mut report = String::from("\n[AST lint]\n");
        for violation in violations {
            report.push_str(&format!(
                "{}:{}:{} [{}] {}\n",
                violation.file.display(),
                violation.line,
                violation.column,
                violation.rule,
                violation.message
            ));
        }
        report
    }
}

#[derive(Debug, Error)]
pub enum KcfLintError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse Rust syntax in {path}:{line}:{column}: {message}")]
    RustParse {
        path: PathBuf,
        line: usize,
        column: usize,
        message: String,
    },
    #[error("failed to parse TOML in {path}: {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("workspace root could not be resolved from {path}")]
    WorkspaceRoot { path: PathBuf },
}
