use crate::diagnostics::{KdrLintError, Violation};
use crate::span::SpanOps;
use crate::syntax::AttributeOps;
use crate::workspace::WorkspaceModel;
use std::path::PathBuf;
use syn::spanned::Spanned;
use syn::visit::Visit;

pub struct ProhibitedAttributeRule;

impl ProhibitedAttributeRule {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KdrLintError> {
        let mut violations = Vec::new();
        for file in workspace.rust_files() {
            let mut visitor = ProhibitedAttributeVisitor::new(file.path().to_path_buf());
            visitor.visit_file(file.syntax());
            violations.extend(visitor.into_violations());
        }
        Ok(violations)
    }
}

struct ProhibitedAttributeVisitor {
    file: PathBuf,
    violations: Vec<Violation>,
}

impl ProhibitedAttributeVisitor {
    fn new(file: PathBuf) -> Self {
        Self {
            file,
            violations: Vec::new(),
        }
    }

    fn into_violations(self) -> Vec<Violation> {
        self.violations
    }

    fn check_attr(&mut self, attr: &syn::Attribute) {
        if !AttributeOps::is_allow_attr(attr) {
            return;
        }
        let location = SpanOps::start(attr.span());
        self.violations.push(Violation::new(
            self.file.clone(),
            location.line,
            location.column,
            "prohibited-attribute",
            "`allow` attributes hide lint failures. Fix the rule violation instead.",
        ));
    }
}

impl<'ast> Visit<'ast> for ProhibitedAttributeVisitor {
    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        self.check_attr(node);
        syn::visit::visit_attribute(self, node);
    }
}
