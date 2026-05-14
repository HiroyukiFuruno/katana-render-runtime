use crate::diagnostics::{KdrLintError, Violation};
use crate::span::SpanOps;
use crate::workspace::WorkspaceModel;
use std::path::PathBuf;
use syn::visit::Visit;

pub struct ProhibitedMethodRule;

impl ProhibitedMethodRule {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KdrLintError> {
        let mut violations = Vec::new();
        for file in workspace.rust_files() {
            let mut visitor = ProhibitedMethodVisitor::new(file.path().to_path_buf());
            visitor.visit_file(file.syntax());
            violations.extend(visitor.into_violations());
        }
        Ok(violations)
    }
}

struct ProhibitedMethodVisitor {
    file: PathBuf,
    violations: Vec<Violation>,
}

impl ProhibitedMethodVisitor {
    fn new(file: PathBuf) -> Self {
        Self {
            file,
            violations: Vec::new(),
        }
    }

    fn into_violations(self) -> Vec<Violation> {
        self.violations
    }

    fn is_prohibited(name: &str) -> bool {
        matches!(
            name,
            "unwrap" | "unwrap_err" | "expect" | "expect_err" | "unwrap_or_default"
        )
    }

    fn push_violation(&mut self, method: &syn::Ident) {
        let location = SpanOps::start(method.span());
        self.violations.push(Violation::new(
            self.file.clone(),
            location.line,
            location.column,
            "prohibited-method",
            format!(
                "method `{method}` hides failure handling. Use explicit Result or Option flow."
            ),
        ));
    }
}

impl<'ast> Visit<'ast> for ProhibitedMethodVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if Self::is_prohibited(&node.method.to_string()) {
            self.push_violation(&node.method);
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}
