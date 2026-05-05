pub struct AttributeOps;

impl AttributeOps {
    pub fn has_cfg_test_attr(attrs: &[syn::Attribute]) -> bool {
        for attr in attrs {
            if Self::is_test_attr(attr) {
                return true;
            }
        }
        false
    }

    pub fn is_allow_attr(attr: &syn::Attribute) -> bool {
        if attr.path().is_ident("allow") {
            return true;
        }

        let syn::Meta::List(list) = &attr.meta else {
            return false;
        };

        if !list.path.is_ident("cfg_attr") {
            return false;
        }
        list.tokens.to_string().contains("allow")
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

#[cfg(test)]
mod tests {
    use super::AttributeOps;

    #[test]
    fn detects_test_and_allow_attributes_without_fallback() {
        let test_attr: syn::Attribute = syn::parse_quote!(#[test]);
        let cfg_test_attr: syn::Attribute = syn::parse_quote!(#[cfg(test)]);
        let cfg_list_attr: syn::Attribute = syn::parse_quote!(#[cfg(any(test))]);
        let allow_attr: syn::Attribute = syn::parse_quote!(#[allow(dead_code)]);
        let cfg_attr: syn::Attribute = syn::parse_quote!(#[cfg_attr(test, allow(dead_code))]);
        let cfg_without_allow_attr: syn::Attribute =
            syn::parse_quote!(#[cfg_attr(test, deprecated)]);
        let non_allow_list_attr: syn::Attribute = syn::parse_quote!(#[cfg(any(unix, test))]);
        let cfg_feature_attr: syn::Attribute = syn::parse_quote!(#[cfg(feature = "demo")]);
        let derive_attr: syn::Attribute = syn::parse_quote!(#[derive(Debug)]);

        assert!(!AttributeOps::is_allow_attr(&test_attr));
        assert!(!AttributeOps::has_cfg_test_attr(&[]));
        assert!(AttributeOps::has_cfg_test_attr(&[test_attr]));
        assert!(AttributeOps::has_cfg_test_attr(&[cfg_test_attr]));
        assert!(!AttributeOps::has_cfg_test_attr(&[cfg_list_attr]));
        assert!(!AttributeOps::has_cfg_test_attr(&[cfg_feature_attr]));
        assert!(!AttributeOps::has_cfg_test_attr(&[derive_attr]));
        assert!(AttributeOps::is_allow_attr(&allow_attr));
        assert!(AttributeOps::is_allow_attr(&cfg_attr));
        assert!(!AttributeOps::is_allow_attr(&cfg_without_allow_attr));
        assert!(!AttributeOps::is_allow_attr(&non_allow_list_attr));
    }
}
