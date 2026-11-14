use serde_json::Value;
use syn::spanned::Spanned;

use super::source::{path_name, record_fact};

const OWNER: &str = "MathJaxGeneratedRuntimeAsset";
pub(super) const MATERIALIZE_STATEMENT_COUNT: usize = 5;
pub(super) const SOURCE_WRITE_INDEX: usize = 3;
pub(super) const RETURNED_PATH_INDEX: usize = 4;
pub(super) const SAME_BYTES_ARM_COUNT: usize = 3;

pub(super) fn require_generated_asset(
    file: &syn::File,
    helper: &syn::File,
    facts: &mut Vec<Value>,
) -> Result<(), String> {
    let item = asset_struct(file)?;
    if !matches!(item.fields, syn::Fields::Unit)
        || !item.generics.params.is_empty()
        || !item.attrs.is_empty()
        || !matches!(item.vis, syn::Visibility::Inherited)
    {
        return Err("default-materializer:asset-type".into());
    }
    if !canonical_materializer_impl(file, OWNER) {
        return Err("default-materializer:owner-context".into());
    }
    validate_materialize(file)?;
    validate_readback(file, helper)?;
    record_fact(
        facts,
        "default-materializer",
        super::source::RUST_INPUTS[0],
        item.span(),
    );
    Ok(())
}

fn validate_materialize(file: &syn::File) -> Result<(), String> {
    let materialize = super::source::method(file, OWNER, "materialize_at")?;
    if !materialize.attrs.is_empty()
        || !path_result_signature(
            &materialize.sig,
            "materialize_at",
            "PathBuf",
            "PathBuf",
            "String",
        )
    {
        return Err("default-materializer:materialize-signature".into());
    }
    super::materializer_body::materialize_body(&materialize.block)?;
    Ok(())
}

fn validate_readback(file: &syn::File, helper: &syn::File) -> Result<(), String> {
    let existing = super::source::method(file, OWNER, "exists_with_same_bytes")?;
    if !existing.attrs.is_empty()
        || !path_result_signature(
            &existing.sig,
            "exists_with_same_bytes",
            "Path",
            "bool",
            "String",
        )
    {
        return Err("default-materializer:same-bytes-signature".into());
    }
    super::materializer_readback::readback(file, helper, &existing.block)?;
    Ok(())
}

fn asset_struct(file: &syn::File) -> Result<&syn::ItemStruct, String> {
    let found = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Struct(item) if item.ident == OWNER => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    match found.as_slice() {
        [item] => Ok(item),
        [] => Err("default-materializer:missing-struct".into()),
        _ => Err("default-materializer:ambiguous-struct".into()),
    }
}

fn path_result_signature(
    signature: &syn::Signature,
    name: &str,
    input: &str,
    ok: &str,
    error: &str,
) -> bool {
    if signature.ident != name || signature.inputs.len() != 1 {
        return false;
    }
    let Some(syn::FnArg::Typed(argument)) = signature.inputs.first() else {
        return false;
    };
    path_argument(argument, input) && result_types(&signature.output, ok, error)
}

fn path_argument(argument: &syn::PatType, input: &str) -> bool {
    matches!(&*argument.pat, syn::Pat::Ident(binding) if binding.ident == "path")
        && type_name(&argument.ty).is_some_and(|actual| actual == input)
}

pub(super) fn result_types(output: &syn::ReturnType, ok: &str, error: &str) -> bool {
    let syn::ReturnType::Type(_, result) = output else {
        return false;
    };
    let syn::Type::Path(result) = result.as_ref() else {
        return false;
    };
    if path_name(&result.path) != "Result" {
        return false;
    }
    let Some(syn::PathArguments::AngleBracketed(arguments)) =
        result.path.segments.last().map(|s| &s.arguments)
    else {
        return false;
    };
    let mut types = arguments.args.iter().filter_map(|argument| match argument {
        syn::GenericArgument::Type(value) => type_name(value),
        _ => None,
    });
    types.next() == Some(ok.to_string())
        && types.next() == Some(error.to_string())
        && types.next().is_none()
}

fn type_name(value: &syn::Type) -> Option<String> {
    match value {
        syn::Type::Path(value) => Some(path_name(&value.path)),
        syn::Type::Reference(value) if value.mutability.is_none() => type_name(&value.elem),
        _ => None,
    }
}

pub(super) fn canonical_materializer_impl(file: &syn::File, owner: &str) -> bool {
    let Some(names) = materializer_methods(owner) else {
        return false;
    };
    let candidates = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(item) => Some(item),
            _ => None,
        })
        .filter(|item| {
            matches!(item.self_ty.as_ref(), syn::Type::Path(path)
        if path.path.segments.last().is_some_and(|segment| segment.ident == owner))
                || item.items.iter().any(|member| {
                    matches!(member, syn::ImplItem::Fn(function)
            if names.iter().any(|name| function.sig.ident == *name))
                })
        })
        .collect::<Vec<_>>();
    let [item] = candidates.as_slice() else {
        return false;
    };
    file.attrs.is_empty() && item.attrs.is_empty() && item.generics.params.is_empty()
        && item.generics.where_clause.is_none() && item.trait_.is_none()
        && matches!(item.self_ty.as_ref(), syn::Type::Path(path) if path.qself.is_none() && path.path.is_ident(owner))
        && item.items.iter().all(|member| matches!(member, syn::ImplItem::Fn(_)))
        && names.iter().all(|name| item.items.iter().filter(|member|
            matches!(member, syn::ImplItem::Fn(function) if function.sig.ident == *name)).count() == 1)
}

fn materializer_methods(owner: &str) -> Option<&'static [&'static str]> {
    match owner {
        "MathJaxGeneratedRuntimeAsset" => Some(&["materialize_at", "exists_with_same_bytes"]),
        "RuntimeAsset" => Some(&["exists_with_matching_bytes"]),
        "MathJaxRendererOps" => Some(&[
            "resolve_mathjax_js",
            "resolve_mathjax_js_with_env",
            "env_mathjax_js_from",
            "default_install_path",
        ]),
        _ => None,
    }
}
