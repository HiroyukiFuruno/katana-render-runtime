pub struct AttributeOps;

impl AttributeOps {
    pub fn has_cfg_test_attr(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(Self::is_test_attr)
    }

    pub fn is_allow_attr(attr: &syn::Attribute) -> bool {
        if attr.path().is_ident("allow") {
            return true;
        }

        let syn::Meta::List(list) = &attr.meta else {
            return false;
        };

        list.path.is_ident("cfg_attr") && list.tokens.to_string().contains("allow")
    }

    fn is_test_attr(attr: &syn::Attribute) -> bool {
        if attr.path().is_ident("test") {
            return true;
        }

        if !attr.path().is_ident("cfg") {
            return false;
        }

        let Ok(syn::Meta::Path(path)) = attr.parse_args::<syn::Meta>() else {
            return false;
        };
        path.is_ident("test")
    }
}
