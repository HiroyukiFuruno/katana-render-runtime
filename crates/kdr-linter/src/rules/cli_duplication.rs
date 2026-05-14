use crate::diagnostics::Violation;
use crate::span::SpanOps;
use crate::workspace::WorkspaceModel;
use std::path::PathBuf;
use syn::visit::Visit;

use super::architecture::CLI_CRATE;

const LIB_API_NAMES: &[&str] = &[
    "DiagramKind",
    "RenderConfig",
    "RenderContext",
    "RenderDiagnostics",
    "RenderError",
    "RenderInput",
    "RenderOutput",
    "RenderPolicy",
    "Renderer",
    "RendererProfile",
    "RuntimeVersion",
];

pub(super) struct CliDuplicationRule;

impl CliDuplicationRule {
    pub(super) fn check(workspace: &WorkspaceModel) -> Vec<Violation> {
        let cli_src = workspace.root().join(CLI_CRATE).join("src");
        let mut violations = Vec::new();
        for file in workspace.rust_files() {
            if !file.is_under(&cli_src) {
                continue;
            }
            let mut visitor = CliDuplicationVisitor::new(file.path().to_path_buf());
            visitor.visit_file(file.syntax());
            violations.extend(visitor.into_violations());
        }
        violations
    }
}

struct CliDuplicationVisitor {
    file: PathBuf,
    violations: Vec<Violation>,
}

impl CliDuplicationVisitor {
    fn new(file: PathBuf) -> Self {
        Self {
            file,
            violations: Vec::new(),
        }
    }

    fn into_violations(self) -> Vec<Violation> {
        self.violations
    }

    fn check_ident(&mut self, ident: &syn::Ident) {
        if !LIB_API_NAMES.iter().any(|it| ident == it) {
            return;
        }
        let location = SpanOps::start(ident.span());
        self.violations.push(Violation::new(
            self.file.clone(),
            location.line,
            location.column,
            "cli-renderer-duplication",
            format!("CLI must reuse library API `{ident}` instead of redeclaring it."),
        ));
    }

    fn check_module_name(&mut self, ident: &syn::Ident) {
        if !matches!(ident.to_string().as_str(), "renderer" | "runtime") {
            return;
        }
        let location = SpanOps::start(ident.span());
        self.violations.push(Violation::new(
            self.file.clone(),
            location.line,
            location.column,
            "cli-renderer-duplication",
            format!("CLI module `{ident}` would own runtime logic. Put it in the library crate."),
        ));
    }
}

impl<'ast> Visit<'ast> for CliDuplicationVisitor {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        self.check_ident(&node.ident);
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        self.check_ident(&node.ident);
        syn::visit::visit_item_enum(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        self.check_ident(&node.ident);
        syn::visit::visit_item_trait(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        self.check_module_name(&node.ident);
        syn::visit::visit_item_mod(self, node);
    }
}
