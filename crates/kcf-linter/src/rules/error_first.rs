use crate::diagnostics::{KcfLintError, Violation};
use crate::span::SpanOps;
use crate::workspace::WorkspaceModel;
use std::path::PathBuf;
use syn::visit::Visit;

pub struct ErrorFirstRule;

impl ErrorFirstRule {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KcfLintError> {
        let mut violations = Vec::new();
        for file in workspace.rust_files() {
            let mut visitor = ErrorFirstVisitor::new(file.path().to_path_buf());
            visitor.visit_file(file.syntax());
            violations.extend(visitor.into_violations());
        }
        Ok(violations)
    }
}

struct ErrorFirstVisitor {
    file: PathBuf,
    violations: Vec<Violation>,
}

impl ErrorFirstVisitor {
    fn new(file: PathBuf) -> Self {
        Self {
            file,
            violations: Vec::new(),
        }
    }

    fn into_violations(self) -> Vec<Violation> {
        self.violations
    }

    fn check_if_let_ok(&mut self, node: &syn::ExprIf) {
        let syn::Expr::Let(let_expr) = &*node.cond else {
            return;
        };
        if !Self::is_ok_pattern(&let_expr.pat) {
            return;
        }
        let location = SpanOps::start(let_expr.let_token.span);
        self.violations.push(Violation::new(
            self.file.clone(),
            location.line,
            location.column,
            "error-first",
            "Do not nest success paths with `if let Ok(...)`. Return errors at the boundary.",
        ));
    }

    fn is_ok_pattern(pattern: &syn::Pat) -> bool {
        match pattern {
            syn::Pat::TupleStruct(tuple) => Self::path_ends_with_ok(&tuple.path),
            syn::Pat::Path(path) => Self::path_ends_with_ok(&path.path),
            _ => false,
        }
    }

    fn path_ends_with_ok(path: &syn::Path) -> bool {
        path.segments.last().is_some_and(|it| it.ident == "Ok")
    }
}

impl<'ast> Visit<'ast> for ErrorFirstVisitor {
    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.check_if_let_ok(node);
        syn::visit::visit_expr_if(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::ErrorFirstVisitor;

    #[test]
    fn ok_pattern_accepts_path_patterns() {
        let pattern = syn::Pat::Path(syn::PatPath {
            attrs: Vec::new(),
            qself: None,
            path: syn::parse_quote!(std::result::Result::Ok),
        });

        assert!(ErrorFirstVisitor::is_ok_pattern(&pattern));
    }

    #[test]
    fn ok_pattern_rejects_wildcard_patterns() {
        let pattern: syn::Pat = syn::parse_quote!(_);

        assert!(!ErrorFirstVisitor::is_ok_pattern(&pattern));
    }

    #[test]
    fn check_if_let_ok_ignores_plain_conditions() {
        let mut visitor = ErrorFirstVisitor::new("fake.rs".into());
        let expr: syn::ExprIf = syn::parse_quote!(if ready {
            done();
        });

        visitor.check_if_let_ok(&expr);

        assert!(visitor.into_violations().is_empty());
    }

    #[test]
    fn check_if_let_ok_ignores_non_ok_patterns() {
        let mut visitor = ErrorFirstVisitor::new("fake.rs".into());
        let expr: syn::ExprIf = syn::parse_quote!(if let Some(value) = result {
            drop(value);
        });

        visitor.check_if_let_ok(&expr);

        assert!(visitor.into_violations().is_empty());
    }
}
