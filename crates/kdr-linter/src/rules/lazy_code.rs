use crate::diagnostics::{KdrLintError, Violation};
use crate::span::SpanOps;
use crate::workspace::WorkspaceModel;
use std::path::PathBuf;
use syn::spanned::Spanned;
use syn::visit::Visit;

pub struct LazyCodeRule;

impl LazyCodeRule {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KdrLintError> {
        let mut violations = Vec::new();
        for file in workspace.rust_files() {
            let mut visitor = LazyCodeVisitor::new(file.path().to_path_buf());
            visitor.visit_file(file.syntax());
            violations.extend(visitor.into_violations());
        }
        Ok(violations)
    }
}

struct LazyCodeVisitor {
    file: PathBuf,
    violations: Vec<Violation>,
}

impl LazyCodeVisitor {
    fn new(file: PathBuf) -> Self {
        Self {
            file,
            violations: Vec::new(),
        }
    }

    fn into_violations(self) -> Vec<Violation> {
        self.violations
    }

    fn prohibited_macro_name(mac: &syn::Macro) -> Option<&'static str> {
        if mac.path.is_ident("todo") {
            return Some("todo");
        }
        if mac.path.is_ident("unimplemented") {
            return Some("unimplemented");
        }
        if mac.path.is_ident("dbg") {
            return Some("dbg");
        }
        None
    }

    fn check_macro(&mut self, mac: &syn::Macro) {
        let Some(name) = Self::prohibited_macro_name(mac) else {
            return;
        };
        let location = SpanOps::start(mac.path.span());
        self.violations.push(Violation::new(
            self.file.clone(),
            location.line,
            location.column,
            "lazy-code",
            format!("macro `{name}!` is prohibited in committed code."),
        ));
    }
}

impl<'ast> Visit<'ast> for LazyCodeVisitor {
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        self.check_macro(node);
        syn::visit::visit_macro(self, node);
    }
}
