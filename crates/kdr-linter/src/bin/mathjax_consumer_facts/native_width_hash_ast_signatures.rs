use syn::{FnArg, Type};

pub(super) fn named_reference(argument: &FnArg, name: &str, target: &str, mutable: bool) -> bool {
    let FnArg::Typed(argument) = argument else {
        return false;
    };
    if !matches!(&*argument.pat, syn::Pat::Ident(pattern) if pattern.ident == name) {
        return false;
    }
    let Type::Reference(reference) = &*argument.ty else {
        return false;
    };
    reference.mutability.is_some() == mutable && type_path_is(&reference.elem, target)
}

pub(super) fn named_value(argument: &FnArg, name: &str, target: &str) -> bool {
    let FnArg::Typed(argument) = argument else {
        return false;
    };
    matches!(&*argument.pat, syn::Pat::Ident(pattern) if pattern.ident == name)
        && type_path_is(&argument.ty, target)
}

pub(super) fn plugin_key_signature(argument: &FnArg) -> bool {
    let FnArg::Typed(argument) = argument else {
        return false;
    };
    if !matches!(&*argument.pat, syn::Pat::Ident(pattern) if pattern.ident == "zenuml_plugin_key") {
        return false;
    }
    let Type::Tuple(tuple) = &*argument.ty else {
        return false;
    };
    tuple.elems.len() == 2
        && tuple.elems.iter().all(|element| {
            matches!(element, Type::Reference(reference) if type_path_is(&reference.elem, "str"))
        })
}

pub(super) fn return_type(output: &syn::ReturnType, expected: &str) -> bool {
    matches!(output, syn::ReturnType::Type(_, ty) if type_path_is(ty, expected))
}

pub(super) fn type_path_is(ty: &Type, expected: &str) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    path.path.segments.len() == 1
        && path
            .path
            .segments
            .first()
            .is_some_and(|segment| segment.ident == expected)
}
