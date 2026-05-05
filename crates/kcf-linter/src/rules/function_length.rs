use crate::diagnostics::{KcfLintError, Violation};
use crate::span::SpanOps;
use crate::workspace::WorkspaceModel;
use std::path::PathBuf;
use syn::visit::Visit;

const MAX_FUNCTION_LINES: usize = 30;

pub struct FunctionLengthRule;

impl FunctionLengthRule {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KcfLintError> {
        let mut violations = Vec::new();
        for file in workspace.rust_files() {
            let mut visitor = FunctionLengthVisitor::new(file.path().to_path_buf());
            visitor.visit_file(file.syntax());
            violations.extend(visitor.into_violations());
        }
        Ok(violations)
    }
}

struct FunctionLengthVisitor {
    file: PathBuf,
    violations: Vec<Violation>,
}

impl FunctionLengthVisitor {
    fn new(file: PathBuf) -> Self {
        Self {
            file,
            violations: Vec::new(),
        }
    }

    fn into_violations(self) -> Vec<Violation> {
        self.violations
    }

    fn check_block(&mut self, name: &syn::Ident, block: &syn::Block) {
        let start_line = name.span().start().line;
        let end_line = SpanOps::block_end_line(block);
        let lines = end_line.saturating_sub(start_line) + 1;
        if lines <= MAX_FUNCTION_LINES {
            return;
        }
        let location = SpanOps::start(name.span());
        self.violations.push(Violation::new(
            self.file.clone(),
            location.line,
            location.column,
            "function-length",
            format!("function `{name}` has {lines} lines. Extract focused helper methods."),
        ));
    }
}

impl<'ast> Visit<'ast> for FunctionLengthVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.check_block(&node.sig.ident, &node.block);
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.check_block(&node.sig.ident, &node.block);
        syn::visit::visit_impl_item_fn(self, node);
    }
}
