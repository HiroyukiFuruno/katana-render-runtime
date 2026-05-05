use crate::diagnostics::{KcfLintError, Violation};
use crate::workspace::{SourceFile, WorkspaceModel};
use syn::{ImplItem, Item};

const MAX_LENGTH_FOR_MIXED_FILE: usize = 250;

pub struct TypeSeparationRule;

impl TypeSeparationRule {
    pub fn check(workspace: &WorkspaceModel) -> Result<Vec<Violation>, KcfLintError> {
        let mut violations = Vec::new();
        for file in workspace.rust_files() {
            if Self::is_exempt(file) {
                continue;
            }
            if let Some(line) = Self::first_mixed_public_type_line(file.syntax()) {
                violations.push(Self::violation(file, line));
            }
        }
        Ok(violations)
    }

    fn is_exempt(file: &SourceFile) -> bool {
        Self::is_exempt_path(&Self::normalized_path(file))
            || file.source().lines().count() <= MAX_LENGTH_FOR_MIXED_FILE
    }

    fn normalized_path(file: &SourceFile) -> String {
        file.path().to_string_lossy().replace('\\', "/")
    }

    fn is_exempt_path(path: &str) -> bool {
        path.contains("/tests/")
            || path.ends_with("tests.rs")
            || path.ends_with("types.rs")
            || path.ends_with("type.rs")
            || path.ends_with("models.rs")
            || path.ends_with("model.rs")
            || path.ends_with("state.rs")
            || path.contains("/types/")
            || path.contains("/models/")
            || path.contains("/state/")
            || path.ends_with("lib.rs")
            || path.ends_with("main.rs")
            || path.ends_with("defaults.rs")
            || path.ends_with("service.rs")
            || path.ends_with("repository.rs")
            || path.ends_with("loader.rs")
            || path.contains("/migration/")
    }

    fn first_mixed_public_type_line(syntax: &syn::File) -> Option<usize> {
        let mut first_public_type_line = None;
        let mut has_logic_impl = false;
        for item in &syntax.items {
            first_public_type_line =
                first_public_type_line.or_else(|| Self::public_type_line(item));
            has_logic_impl |= Self::has_logic_impl(item);
        }
        has_logic_impl.then_some(first_public_type_line).flatten()
    }

    fn public_type_line(item: &Item) -> Option<usize> {
        match item {
            Item::Struct(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.span().start().line)
            }
            Item::Enum(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.span().start().line)
            }
            _ => None,
        }
    }

    fn has_logic_impl(item: &Item) -> bool {
        match item {
            Item::Impl(item) => item.items.iter().any(|it| matches!(it, ImplItem::Fn(_))),
            _ => false,
        }
    }

    fn violation(file: &SourceFile, line: usize) -> Violation {
        let lines = file.source().lines().count();
        Violation::new(
            file.path().to_path_buf(),
            line,
            1,
            "type-separation",
            format!(
                "file has {lines} lines and mixes public types with implementation logic. Move public data shapes to `types.rs`."
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::TypeSeparationRule;

    #[test]
    fn public_type_line_accepts_public_enum() {
        let item: syn::Item = syn::parse_quote!(
            pub enum PublicShape {
                One,
            }
        );

        assert!(TypeSeparationRule::public_type_line(&item).is_some());
    }

    #[test]
    fn public_type_line_accepts_public_struct() {
        let item: syn::Item = syn::parse_quote!(
            pub struct PublicShape;
        );

        assert!(TypeSeparationRule::public_type_line(&item).is_some());
    }

    #[test]
    fn public_type_line_rejects_private_enum() {
        let item: syn::Item = syn::parse_quote!(
            enum PrivateShape {
                One,
            }
        );

        assert!(TypeSeparationRule::public_type_line(&item).is_none());
    }

    #[test]
    fn first_mixed_public_type_line_accepts_type_with_impl() {
        let syntax: syn::File = syn::parse_quote! {
            pub struct PublicShape;
            impl PublicShape {
                fn render(&self) {}
            }
        };

        assert!(TypeSeparationRule::first_mixed_public_type_line(&syntax).is_some());
    }

    #[test]
    fn first_mixed_public_type_line_rejects_type_without_impl() {
        let syntax: syn::File = syn::parse_quote!(
            pub struct PublicShape;
        );

        assert!(TypeSeparationRule::first_mixed_public_type_line(&syntax).is_none());
    }
}
