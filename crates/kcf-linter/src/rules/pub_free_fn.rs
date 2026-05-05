use crate::diagnostics::{KcfLintError, Violation};
use crate::span::SpanOps;
use crate::syntax::AttributeOps;
use crate::workspace::WorkspaceModel;
use std::path::PathBuf;
use syn::visit::Visit;

pub struct PublicFreeFunctionRule;

impl PublicFreeFunctionRule {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KcfLintError> {
        let mut violations = Vec::new();
        for file in workspace.rust_files() {
            let mut visitor = PublicFreeFunctionVisitor::new(file.path().to_path_buf());
            visitor.visit_file(file.syntax());
            violations.extend(visitor.into_violations());
        }
        Ok(violations)
    }
}

struct PublicFreeFunctionVisitor {
    file: PathBuf,
    violations: Vec<Violation>,
    in_test_context: bool,
}

impl PublicFreeFunctionVisitor {
    fn new(file: PathBuf) -> Self {
        Self {
            file,
            violations: Vec::new(),
            in_test_context: false,
        }
    }

    fn into_violations(self) -> Vec<Violation> {
        self.violations
    }

    fn is_public(vis: &syn::Visibility) -> bool {
        matches!(vis, syn::Visibility::Public(_))
            || matches!(vis, syn::Visibility::Restricted(scope) if scope.path.is_ident("crate"))
    }

    fn push_violation(&mut self, node: &syn::ItemFn) {
        let location = SpanOps::start(node.sig.ident.span());
        self.violations.push(Violation::new(
            self.file.clone(),
            location.line,
            location.column,
            "public-free-function",
            format!(
                "public free function `{}` must move behind a struct impl.",
                node.sig.ident
            ),
        ));
    }
}

impl<'ast> Visit<'ast> for PublicFreeFunctionVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let previous = self.in_test_context;
        self.in_test_context |= AttributeOps::has_cfg_test_attr(&node.attrs);
        syn::visit::visit_item_mod(self, node);
        self.in_test_context = previous;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if self.in_test_context || node.sig.ident == "main" {
            return;
        }
        if AttributeOps::has_cfg_test_attr(&node.attrs) {
            return;
        }
        if Self::is_public(&node.vis) {
            self.push_violation(node);
        }
        syn::visit::visit_item_fn(self, node);
    }
}
