use super::{ident, method, method_call, tail};
use syn::Expr;

const IDENTITY_FIELDS: usize = 4;
const UNIX_IDENTITY_FIELDS: usize = 4;

pub(super) fn inspect(file: &syn::File) -> Result<(), String> {
    let definitions = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Struct(value) if value.ident == "Identity" => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [identity] = definitions.as_slice() else {
        return Err("resource-loader:metadata".into());
    };
    if !identity.attrs.iter().any(|attr| {
        attr.path().is_ident("derive")
            && attr
                .meta
                .require_list()
                .is_ok_and(|list| list.tokens.to_string() == "Clone , PartialEq , Eq")
    }) {
        return Err("resource-loader:metadata".into());
    }
    require_identity_metadata(file)
}

fn require_identity_metadata(file: &syn::File) -> Result<(), String> {
    let identity = method(file, "Identity", "from_metadata")?;
    let Expr::Struct(value) = tail(&identity.block)? else {
        return Err("resource-loader:metadata".into());
    };
    if super::super::super::super::source::path_name(&value.path) != "Self"
        || value.rest.is_some()
        || value.fields.len() != IDENTITY_FIELDS
    {
        return Err("resource-loader:metadata".into());
    }
    require_identity_fields(value)?;
    require_identity_unix(value)
}

fn require_identity_fields(value: &syn::ExprStruct) -> Result<(), String> {
    for name in ["len", "modified", "created"] {
        let Some(field) = value
            .fields
            .iter()
            .find(|field| matches!(&field.member, syn::Member::Named(actual) if actual == name))
        else {
            return Err("resource-loader:metadata".into());
        };
        let expression = metadata_field_expression(&field.expr, name)?;
        if !matches!(method_call(expression), Some(read)
            if read.method == name && read.args.is_empty() && ident(&read.receiver, "metadata"))
        {
            return Err("resource-loader:metadata".into());
        }
    }
    Ok(())
}

fn metadata_field_expression<'a>(expression: &'a Expr, name: &str) -> Result<&'a Expr, String> {
    if name == "len" {
        return Ok(expression);
    }
    let Some(ok) = method_call(expression) else {
        return Err("resource-loader:metadata".into());
    };
    if ok.method != "ok" || !ok.args.is_empty() {
        return Err("resource-loader:metadata".into());
    }
    Ok(&ok.receiver)
}

fn require_identity_unix(value: &syn::ExprStruct) -> Result<(), String> {
    let Some(unix) = value
        .fields
        .iter()
        .find(|field| matches!(&field.member, syn::Member::Named(actual) if actual == "unix"))
    else {
        return Err("resource-loader:metadata".into());
    };
    if !matches!(&unix.expr, Expr::Tuple(tuple)
        if tuple.elems.len() == UNIX_IDENTITY_FIELDS && tuple.elems.iter().zip(["dev", "ino", "ctime", "ctime_nsec"])
            .all(|(item, name)| matches!(method_call(item), Some(value)
                if value.method == name && value.args.is_empty() && ident(&value.receiver, "metadata"))))
    {
        return Err("resource-loader:metadata".into());
    }
    Ok(())
}
