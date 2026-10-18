use super::materializer_body::call_expr;
use super::source::{call, ident, path_name, path_name_from_expr};

#[path = "materializer_shared_readback.rs"]
mod shared;

pub(super) fn readback(
    file: &syn::File,
    helper: &syn::File,
    block: &syn::Block,
) -> Result<(), String> {
    if matches!(
        block.stmts.as_slice(),
        [syn::Stmt::Expr(syn::Expr::Match(_), _)]
    ) {
        return same_bytes_body(block);
    }
    shared::inspect(file, helper, block)
}

pub(super) fn same_bytes_body(block: &syn::Block) -> Result<(), String> {
    let [syn::Stmt::Expr(syn::Expr::Match(expression), _)] = block.stmts.as_slice() else {
        return Err("default-materializer:read-shape".into());
    };
    if !call_expr(&expression.expr, "std::fs::read", &["path"])
        || expression.arms.len() != super::materializer::SAME_BYTES_ARM_COUNT
    {
        return Err("default-materializer:read-shape".into());
    }
    if !matches_read_arm(&expression.arms[0]) {
        return Err("default-materializer:read-bytes-match".into());
    }
    if !matches_not_found_arm(&expression.arms[1]) {
        return Err("default-materializer:not-found-match".into());
    }
    if !matches_error_arm(&expression.arms[2]) {
        return Err("default-materializer:read-error-match".into());
    }
    Ok(())
}

fn matches_read_arm(arm: &syn::Arm) -> bool {
    let (pattern, guard) = pattern_and_guard(&arm.pat);
    let syn::Pat::TupleStruct(pattern) = pattern else {
        return false;
    };
    path_name(&pattern.path) == "Ok"
        && matches!(pattern.elems.first(), Some(syn::Pat::Ident(id)) if id.ident == "existing")
        && pattern.elems.len() == 1
        && guard.is_none()
        && matches_source_bytes(&arm.body)
}

pub(super) fn matches_source_bytes(expression: &syn::Expr) -> bool {
    let Some(ok) = call(expression) else {
        return false;
    };
    if path_name_from_expr(&ok.func) != "Ok" || ok.args.len() != 1 {
        return false;
    }
    let syn::Expr::Binary(equality) = &ok.args[0] else {
        return false;
    };
    let syn::BinOp::Eq(_) = equality.op else {
        return false;
    };
    ident(&equality.left, "existing")
        && matches!(&*equality.right, syn::Expr::MethodCall(as_bytes) if as_bytes.method == "as_bytes" && as_bytes.args.is_empty() && call_expr(&as_bytes.receiver, "MathJaxRuntimeScripts::runtime_source", &[]))
}

fn matches_not_found_arm(arm: &syn::Arm) -> bool {
    let (pattern, guard) = pattern_and_guard(&arm.pat);
    let syn::Pat::TupleStruct(pattern) = pattern else {
        return false;
    };
    let Some(syn::Expr::Binary(guard)) = guard else {
        return false;
    };
    path_name(&pattern.path) == "Err"
        && matches!(pattern.elems.first(), Some(syn::Pat::Ident(id)) if id.ident == "error")
        && pattern.elems.len() == 1
        && matches!(guard.op, syn::BinOp::Eq(_))
        && matches!(&*guard.left, syn::Expr::MethodCall(kind) if kind.method == "kind" && kind.args.is_empty() && ident(&kind.receiver, "error"))
        && ident(&guard.right, "std::io::ErrorKind::NotFound")
        && ok_false(&arm.body)
}

fn ok_false(expression: &syn::Expr) -> bool {
    let Some(ok) = call(expression) else {
        return false;
    };
    path_name_from_expr(&ok.func) == "Ok"
        && ok.args.len() == 1
        && matches!(&ok.args[0], syn::Expr::Lit(literal) if matches!(&literal.lit, syn::Lit::Bool(value) if !value.value))
}

fn matches_error_arm(arm: &syn::Arm) -> bool {
    let (pattern, guard) = pattern_and_guard(&arm.pat);
    let syn::Pat::TupleStruct(pattern) = pattern else {
        return false;
    };
    path_name(&pattern.path) == "Err"
        && matches!(pattern.elems.first(), Some(syn::Pat::Ident(id)) if id.ident == "error")
        && pattern.elems.len() == 1
        && guard.is_none()
        && matches!(&*arm.body, syn::Expr::Call(call) if path_name_from_expr(&call.func) == "Err" && call.args.len() == 1 && call_expr(&call.args[0], "runtime_asset_error", &["error"]))
}

fn pattern_and_guard(pattern: &syn::Pat) -> (&syn::Pat, Option<&syn::Expr>) {
    match pattern {
        syn::Pat::Guard(guard) => (&guard.pat, Some(&guard.guard)),
        pattern => (pattern, None),
    }
}

pub(super) fn plain_binding(pattern: &syn::Pat, name: &str) -> bool {
    matches!(pattern, syn::Pat::Ident(binding) if binding.ident == name && binding.subpat.is_none()
        && binding.by_ref.is_none() && binding.mutability.is_none())
}

pub(super) fn standard_path_binding(file: &syn::File) -> bool {
    let uses = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Use(item) => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    uses.iter()
        .map(|item| shared::imports_name(&item.tree, "Path"))
        .sum::<usize>()
        == 1
        && uses.iter().any(|item| {
            item.attrs.is_empty()
                && matches!(&item.tree,
            syn::UseTree::Path(root) if root.ident == "std" && standard_path_import(&root.tree))
        })
}

fn standard_path_import(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Path(module) if module.ident == "path" => {
            matches!(module.tree.as_ref(), syn::UseTree::Group(group)
                if group.items.iter().any(|item| matches!(item, syn::UseTree::Name(name) if name.ident == "Path")))
        }
        syn::UseTree::Group(group) => group.items.iter().any(|item| {
            matches!(item, syn::UseTree::Path(module) if module.ident == "path")
                && standard_path_import(item)
        }),
        _ => false,
    }
}

pub(super) fn byte_slice_argument(argument: &syn::NamedArg) -> bool {
    argument.attrs.is_empty()
        && argument.name.is_none()
        && matches!(&argument.ty, syn::Type::Reference(value) if value.mutability.is_none()
            && value.lifetime.is_none() && matches!(value.elem.as_ref(), syn::Type::Slice(slice)
                if matches!(slice.elem.as_ref(), syn::Type::Path(path) if path.path.is_ident("u8"))))
}

pub(super) fn import_binding(file: &syn::File) -> Result<(), String> {
    let uses = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Use(item) => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    if uses
        .iter()
        .map(|item| shared::imports_name(&item.tree, "RuntimeAsset"))
        .sum::<usize>()
        != 1
        || !uses
            .iter()
            .any(|item| item.attrs.is_empty() && exact_import(&item.tree))
        || !standard_path_binding(file)
        || shared::conflicting_binding(file, false)
    {
        return Err("default-materializer:shared-import-binding".into());
    }
    Ok(())
}

fn exact_import(tree: &syn::UseTree) -> bool {
    matches!(tree, syn::UseTree::Path(root) if root.ident == "crate" &&
        matches!(root.tree.as_ref(), syn::UseTree::Path(markdown) if markdown.ident == "markdown" &&
            matches!(markdown.tree.as_ref(), syn::UseTree::Path(module) if module.ident == "runtime_assets" &&
                matches!(module.tree.as_ref(), syn::UseTree::Name(name) if name.ident == "RuntimeAsset"))))
}
