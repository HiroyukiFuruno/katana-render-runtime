use super::super::materializer::result_types;
use super::super::source::{ident, method};
use super::plain_binding;
use syn::{Block, Expr, FnArg, Item, Stmt, Type, UseTree};

const EXPECTED_READBACK_ARMS: usize = 3;

pub(super) fn inspect(file: &syn::File, helper: &syn::File, block: &Block) -> Result<(), String> {
    super::import_binding(file)?;
    let [Stmt::Expr(Expr::Call(forward), None)] = block.stmts.as_slice() else {
        return Err("default-materializer:shared-call-shape".into());
    };
    if !ident(&forward.func, "RuntimeAsset::exists_with_matching_bytes")
        || forward.args.len() != 2
        || !ident(&forward.args[0], "path")
    {
        return Err("default-materializer:shared-call".into());
    }
    let expected = predicate(&forward.args[1])?;
    if !super::super::materializer::canonical_materializer_impl(helper, "RuntimeAsset") {
        return Err("default-materializer:helper-binding-signature".into());
    }
    let actual = method(helper, "RuntimeAsset", "exists_with_matching_bytes")?;
    helper_context(helper, actual)?;
    project_readback(&actual.block, expected)
}

fn project_readback(block: &Block, expected: Expr) -> Result<(), String> {
    let mut projected = block.clone();
    let [Stmt::Expr(Expr::Match(read), None)] = projected.stmts.as_mut_slice() else {
        return Err("default-materializer:helper-read-shape".into());
    };
    if read.arms.len() != EXPECTED_READBACK_ARMS
        || !predicate_call(&read.arms[0].body)
        || !error_string(&read.arms[2].body)
    {
        return Err("default-materializer:helper-callback-error".into());
    }
    *read.arms[0].body = expected;
    *read.arms[2].body = syn::parse_str("Err(runtime_asset_error(error))")
        .map_err(|_| "default-materializer:helper-template")?;
    super::same_bytes_body(&projected)
}

fn predicate(expression: &Expr) -> Result<Expr, String> {
    let Expr::Closure(closure) = expression else {
        return Err("default-materializer:shared-predicate".into());
    };
    if closure.inputs.len() != 1
        || !plain_binding(&closure.inputs[0], "existing")
        || closure.asyncness.is_some()
        || closure.capture.is_some()
        || closure.lifetimes.is_some()
        || !matches!(closure.output, syn::ReturnType::Default)
    {
        return Err("default-materializer:shared-predicate".into());
    }
    let Expr::Block(body) = closure.body.as_ref() else {
        return Err("default-materializer:shared-predicate-body".into());
    };
    let [Stmt::Expr(expected, None)] = body.block.stmts.as_slice() else {
        return Err("default-materializer:shared-predicate-body".into());
    };
    if body.label.is_some() || !super::matches_source_bytes(expected) {
        return Err("default-materializer:shared-expected-bytes".into());
    }
    Ok(expected.clone())
}

fn predicate_call(expression: &Expr) -> bool {
    matches!(expression, Expr::Call(call) if ident(&call.func, "matches") && call.args.len() == 1
        && matches!(&call.args[0], Expr::Reference(arg) if arg.mutability.is_none() && ident(&arg.expr, "existing"))
    )
}

fn error_string(expression: &Expr) -> bool {
    matches!(expression, Expr::Call(call) if ident(&call.func, "Err") && call.args.len() == 1
        && matches!(&call.args[0], Expr::MethodCall(value) if value.method == "to_string"
            && value.args.is_empty() && ident(&value.receiver, "error"))
    )
}

fn helper_context(file: &syn::File, function: &syn::ImplItemFn) -> Result<(), String> {
    if !function.attrs.is_empty()
        || !signature(&function.sig)
        || !super::standard_path_binding(file)
        || conflicting_binding(file, true)
    {
        return Err("default-materializer:helper-binding-signature".into());
    }
    Ok(())
}

fn signature(sig: &syn::Signature) -> bool {
    if sig.inputs.len() != 2
        || sig.asyncness.is_some()
        || !matches!(sig.safety, syn::Safety::Default)
        || !sig.generics.params.is_empty()
        || sig.generics.where_clause.is_some()
        || !result_types(&sig.output, "bool", "String")
    {
        return false;
    }
    let (FnArg::Typed(path), FnArg::Typed(callback)) = (&sig.inputs[0], &sig.inputs[1]) else {
        return false;
    };
    if !plain_binding(&path.pat, "path")
        || !plain_binding(&callback.pat, "matches")
        || !matches!(path.ty.as_ref(), Type::Reference(value) if value.mutability.is_none()
            && value.lifetime.is_none() && matches!(value.elem.as_ref(), Type::Path(path) if path.path.is_ident("Path")))
    {
        return false;
    }
    callback_signature(&callback.ty)
}

fn callback_signature(value: &Type) -> bool {
    let Type::ImplTrait(bounds) = value else {
        return false;
    };
    let Some(syn::TypeParamBound::Trait(bound)) = bounds.bounds.first() else {
        return false;
    };
    let Some(segment) = bound.path.segments.first() else {
        return false;
    };
    let syn::PathArguments::Parenthesized(args) = &segment.arguments else {
        return false;
    };
    bounds.bounds.len() == 1
        && bound.path.segments.len() == 1
        && segment.ident == "FnOnce"
        && bound.lifetimes.is_none()
        && bound.modifiers.require_empty().is_ok()
        && bound.maybe.is_none()
        && args.inputs.len() == 1
        && super::byte_slice_argument(&args.inputs[0])
        && result_types(&args.output, "bool", "String")
}

pub(super) fn imports_name(tree: &UseTree, name: &str) -> usize {
    match tree {
        UseTree::Name(value) => usize::from(value.ident == name),
        UseTree::Rename(value) => usize::from(value.rename == name),
        UseTree::Path(value) => imports_name(&value.tree, name),
        UseTree::Group(value) => value
            .items
            .iter()
            .map(|item| imports_name(item, name))
            .sum(),
        UseTree::Glob(_) => 2,
    }
}

pub(super) fn conflicting_binding(file: &syn::File, helper: bool) -> bool {
    let reserved = |name: &syn::Ident| {
        ["RuntimeAsset", "Path", "String", "Result", "FnOnce", "std"]
            .iter()
            .any(|expected| name == expected)
    };
    file.items.iter().any(|item| match item {
        Item::Struct(value) if helper && value.ident == "RuntimeAsset" => {
            !value.attrs.is_empty() || !value.generics.params.is_empty()
        }
        Item::Struct(value) => reserved(&value.ident),
        Item::Enum(value) => reserved(&value.ident),
        Item::Union(value) => reserved(&value.ident),
        Item::Type(value) => reserved(&value.ident),
        Item::Mod(value) => reserved(&value.ident),
        Item::Trait(value) => reserved(&value.ident),
        Item::ExternCrate(value) => reserved(value.rename.as_ref().map_or(&value.ident, |(_, name)| name)),
        Item::Impl(value) if !helper => matches!(value.self_ty.as_ref(), Type::Path(owner) if owner.path.is_ident("RuntimeAsset")),
        Item::Macro(value) => !helper || !test_asset_index(value),
        Item::Use(value) => ["String", "Result", "FnOnce", "std"].iter()
            .any(|name| imports_name(&value.tree, name) > 0)
            || (helper && imports_name(&value.tree, "RuntimeAsset") > 0),
        _ => false,
    }) || (helper && file.items.iter().filter(|item| matches!(item, Item::Struct(value) if value.ident == "RuntimeAsset")).count() != 1)
}

fn test_asset_index(item: &syn::ItemMacro) -> bool {
    item.ident.is_none()
        && item.mac.path.is_ident("include")
        && item.attrs.len() == 1
        && item.attrs[0].path().is_ident("cfg")
        && item.attrs[0]
            .parse_args::<syn::Path>()
            .is_ok_and(|path| path.is_ident("test"))
        && syn::parse2::<syn::LitStr>(item.mac.tokens.clone())
            .is_ok_and(|path| path.value() == "generated/zenuml-runtime-assets-index.rs")
}
