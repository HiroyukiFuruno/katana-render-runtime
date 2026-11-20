use super::super::super::super::super::source::ident;
use super::super::super::super::super::syntax::call_expr_path;
use super::super::super::method_call;
use syn::{Expr, Stmt};

pub(super) fn verify_and_remove_platform_attribute(statement: &mut Stmt) -> bool {
    let Stmt::Expr(Expr::If(branch), None) = statement else {
        return false;
    };
    if !windows_only(&branch.attrs)
        || !rejects_stale_bytes(&branch.cond)
        || branch.else_branch.is_some()
        || !matches!(branch.then_branch.stmts.as_slice(),
            [Stmt::Expr(Expr::Return(value), Some(_))]
                if value.expr.as_deref().is_some_and(|value| ident(value, "None")))
    {
        return false;
    }
    branch.attrs.clear();
    true
}

fn windows_only(attributes: &[syn::Attribute]) -> bool {
    matches!(attributes, [attribute]
        if attribute.path().is_ident("cfg")
            && attribute.parse_args::<syn::Path>().is_ok_and(|path| path.is_ident("windows")))
}

fn rejects_stale_bytes(expression: &Expr) -> bool {
    matches!(expression, Expr::Unary(negated)
        if matches!(negated.op, syn::UnOp::Not(_))
            && matches!(method_call(&negated.expr), Some(check)
                if check.method == "is_ok_and" && check.args.len() == 1
                    && call_expr_path(&check.receiver, "Self::read_file", &["path"])
                    && checks_snapshot_bytes(&check.args[0])))
}

fn checks_snapshot_bytes(expression: &Expr) -> bool {
    matches!(expression, Expr::Closure(compare)
        if compare.inputs.len() == 1
            && matches!(compare.inputs.first(), Some(syn::Pat::Ident(input)) if input.ident == "bytes")
            && matches!(compare.body.as_ref(), Expr::Binary(equal)
                if matches!(equal.op, syn::BinOp::Eq(_)) && ident(&equal.left, "bytes")
                    && matches!(method_call(&equal.right), Some(bytes)
                        if bytes.method == "bytes" && bytes.args.is_empty()
                            && ident(&bytes.receiver, "snapshot"))))
}
