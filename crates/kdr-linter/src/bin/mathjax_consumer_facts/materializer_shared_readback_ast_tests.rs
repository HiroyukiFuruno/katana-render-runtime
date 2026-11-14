use super::callback_signature;

#[test]
fn empty_callback_bounds_and_paths_are_rejected() -> Result<(), syn::Error> {
    let empty = syn::Type::ImplTrait(syn::TypeImplTrait {
        impl_token: Default::default(),
        bounds: Default::default(),
        attrs: Vec::new(),
    });
    assert!(!callback_signature(&empty));
    let mut value: syn::Type = syn::parse_str("impl FnOnce(&[u8]) -> Result<bool, String>")?;
    let syn::Type::ImplTrait(bounds) = &mut value else {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "expected impl trait",
        ));
    };
    let Some(syn::TypeParamBound::Trait(bound)) = bounds.bounds.first_mut() else {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "expected trait bound",
        ));
    };
    bound.path.segments.clear();
    assert!(!callback_signature(&value));
    Ok(())
}
