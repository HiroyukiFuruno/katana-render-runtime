use crate::diagnostics::{KcfLintError, Violation};
use crate::span::SpanOps;
use crate::workspace::WorkspaceModel;
use std::path::PathBuf;
use syn::spanned::Spanned;
use syn::visit::Visit;

pub struct ProhibitedTypeRule;

impl ProhibitedTypeRule {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KcfLintError> {
        let mut violations = Vec::new();
        for file in workspace.rust_files() {
            let mut visitor = ProhibitedTypeVisitor::new(file.path().to_path_buf());
            visitor.visit_file(file.syntax());
            violations.extend(visitor.into_violations());
        }
        Ok(violations)
    }
}

struct ProhibitedTypeVisitor {
    file: PathBuf,
    violations: Vec<Violation>,
}

impl ProhibitedTypeVisitor {
    fn new(file: PathBuf) -> Self {
        Self {
            file,
            violations: Vec::new(),
        }
    }

    fn into_violations(self) -> Vec<Violation> {
        self.violations
    }

    fn check_type_path(&mut self, node: &syn::TypePath) {
        if Self::type_name(node) != "std::sync::RwLock" {
            return;
        }
        let location = SpanOps::start(node.path.span());
        self.violations.push(Violation::new(
            self.file.clone(),
            location.line,
            location.column,
            "prohibited-type",
            "Use an explicit non-poisoning lock instead of `std::sync::RwLock`.",
        ));
    }

    fn type_name(node: &syn::TypePath) -> String {
        node.path
            .segments
            .iter()
            .map(|it| it.ident.to_string())
            .collect::<Vec<_>>()
            .join("::")
    }
}

impl<'ast> Visit<'ast> for ProhibitedTypeVisitor {
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        self.check_type_path(node);
        syn::visit::visit_type_path(self, node);
    }
}
