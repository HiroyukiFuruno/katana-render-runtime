use crate::diagnostics::{KcfLintError, Violation};
use crate::span::SpanOps;
use crate::workspace::WorkspaceModel;
use std::collections::BTreeSet;
use std::path::PathBuf;
use syn::spanned::Spanned;
use syn::visit::Visit;

pub struct ProhibitedTypeRule;

impl ProhibitedTypeRule {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KcfLintError> {
        let mut violations = Vec::new();
        for file in workspace.rust_files() {
            let imports = RwLockImportCollector::collect(file.syntax());
            let mut visitor = ProhibitedTypeVisitor::new(file.path().to_path_buf(), imports);
            visitor.visit_file(file.syntax());
            violations.extend(visitor.into_violations());
        }
        Ok(violations)
    }
}

struct ProhibitedTypeVisitor {
    file: PathBuf,
    imported_rwlock_names: BTreeSet<String>,
    violations: Vec<Violation>,
}

impl ProhibitedTypeVisitor {
    fn new(file: PathBuf, imported_rwlock_names: BTreeSet<String>) -> Self {
        Self {
            file,
            imported_rwlock_names,
            violations: Vec::new(),
        }
    }

    fn into_violations(self) -> Vec<Violation> {
        self.violations
    }

    fn check_type_path(&mut self, node: &syn::TypePath) {
        if !self.is_prohibited_rwlock(node) {
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

    fn is_prohibited_rwlock(&self, node: &syn::TypePath) -> bool {
        let type_name = Self::type_name(node);
        type_name == "std::sync::RwLock" || self.imported_rwlock_names.contains(&type_name)
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

struct RwLockImportCollector {
    imported_names: BTreeSet<String>,
}

impl RwLockImportCollector {
    fn collect(file: &syn::File) -> BTreeSet<String> {
        let mut collector = Self {
            imported_names: BTreeSet::new(),
        };
        collector.visit_file(file);
        collector.imported_names
    }

    fn collect_tree(&mut self, tree: &syn::UseTree, prefix: &mut Vec<String>) {
        match tree {
            syn::UseTree::Path(path) => self.collect_path(path, prefix),
            syn::UseTree::Name(name) => self.collect_name(&name.ident, prefix),
            syn::UseTree::Rename(rename) => self.collect_rename(rename, prefix),
            syn::UseTree::Glob(_) => self.collect_glob(prefix),
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.collect_tree(item, prefix);
                }
            }
        }
    }

    fn collect_path(&mut self, path: &syn::UsePath, prefix: &mut Vec<String>) {
        prefix.push(path.ident.to_string());
        self.collect_tree(&path.tree, prefix);
        prefix.pop();
    }

    fn collect_name(&mut self, ident: &syn::Ident, prefix: &[String]) {
        if Self::is_std_sync(prefix) && ident == "RwLock" {
            self.imported_names.insert("RwLock".to_string());
        }
    }

    fn collect_rename(&mut self, rename: &syn::UseRename, prefix: &[String]) {
        if Self::is_std_sync(prefix) && rename.ident == "RwLock" {
            self.imported_names.insert(rename.rename.to_string());
        }
    }

    fn collect_glob(&mut self, prefix: &[String]) {
        if Self::is_std_sync(prefix) {
            self.imported_names.insert("RwLock".to_string());
        }
    }

    fn is_std_sync(prefix: &[String]) -> bool {
        prefix == ["std", "sync"]
    }
}

impl<'ast> Visit<'ast> for RwLockImportCollector {
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        self.collect_tree(&node.tree, &mut Vec::new());
    }
}

impl<'ast> Visit<'ast> for ProhibitedTypeVisitor {
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        self.check_type_path(node);
        syn::visit::visit_type_path(self, node);
    }
}
