use crate::diagnostics::Violation;
use crate::span::SpanOps;
use crate::workspace::WorkspaceModel;
use proc_macro2::{TokenStream, TokenTree};
use std::path::Path;
use syn::{spanned::Spanned, visit::Visit};

use super::architecture::LIB_CRATE;

const SHARED_FONT_FILES: &[&str] = &[
    "svg_rasterize_text_font_family.rs",
    "svg_rasterize_text_family_runs.rs",
    "svg_rasterize_text_shaping_font_runs.rs",
];

pub(super) fn check(workspace: &WorkspaceModel) -> Vec<Violation> {
    let markdown = workspace.root().join(LIB_CRATE).join("src/markdown");
    workspace
        .rust_files()
        .iter()
        .filter(|file| {
            file.path().parent() == Some(markdown.as_path())
                && file.path().file_name().is_some_and(|name| {
                    SHARED_FONT_FILES.iter().any(|file_name| name == *file_name)
                })
        })
        .flat_map(|file| violations(file.path(), file.syntax()))
        .collect()
}

fn violations(path: &Path, syntax: &syn::File) -> Vec<Violation> {
    let mut visitor = PlatformConditions {
        path,
        found: Vec::new(),
    };
    visitor.visit_file(syntax);
    visitor.found
}

struct PlatformConditions<'a> {
    path: &'a Path,
    found: Vec<Violation>,
}

impl PlatformConditions<'_> {
    fn check(&mut self, span: proc_macro2::Span, tokens: &TokenStream) {
        if has_platform_condition(tokens.clone()) {
            let location = SpanOps::start(span);
            self.found.push(Violation::new(
                self.path.to_path_buf(),
                location.line,
                location.column,
                "font-platform-boundary",
                "Move OS font conditions into PlatformFontStrategy.",
            ));
        }
    }
}

impl<'ast> Visit<'ast> for PlatformConditions<'_> {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if let syn::Meta::List(list) = &attribute.meta
            && (list.path.is_ident("cfg") || list.path.is_ident("cfg_attr"))
        {
            self.check(attribute.span(), &list.tokens);
        }
        syn::visit::visit_attribute(self, attribute);
    }

    fn visit_macro(&mut self, expression: &'ast syn::Macro) {
        if expression.path.is_ident("cfg") {
            self.check(expression.span(), &expression.tokens);
        }
        syn::visit::visit_macro(self, expression);
    }
}

fn has_platform_condition(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Group(group) => has_platform_condition(group.stream()),
        TokenTree::Ident(name) => matches!(
            name.to_string().as_str(),
            "target_os" | "target_family" | "target_vendor" | "target_env" | "windows" | "unix"
        ),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::{Path, violations};

    #[test]
    fn ast_linter_font_platform_boundary_rejects_os_conditions() -> syn::Result<()> {
        let syntax = syn::parse_file(
            r#"#[cfg(all(test, target_os = "linux"))] fn first() {}
#[cfg_attr(not(windows), inline)] fn second() {}
fn third() { if cfg!(target_family = "unix") {} }
#[cfg(test)] fn platform_independent_test() {}"#,
        )?;
        let found = violations(Path::new("shared_fonts.rs"), &syntax);
        assert_eq!(
            found
                .iter()
                .map(|diagnostic| diagnostic.line)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        Ok(())
    }
}
