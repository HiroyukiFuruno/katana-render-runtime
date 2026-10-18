use super::materializer::{MATERIALIZE_STATEMENT_COUNT, RETURNED_PATH_INDEX, SOURCE_WRITE_INDEX};
use super::source::{call, ident, path_name_from_expr};

const NO_PARENT: &str = "MathJax generated runtime path has no parent";

pub(super) fn materialize_body(block: &syn::Block) -> Result<(), String> {
    let statements = &block.stmts;
    if statements.len() != MATERIALIZE_STATEMENT_COUNT {
        return Err("default-materializer:statement-count".into());
    }
    if !same_bytes_guard(statements.first()) {
        return Err("default-materializer:existing-byte-guard".into());
    }
    if !parent_binding(statements.get(1)) {
        return Err("default-materializer:parent-binding".into());
    }
    if !fs_call(statements.get(2), "std::fs::create_dir_all", &["parent"]) {
        return Err("default-materializer:create-parent".into());
    }
    if !write_call(statements.get(SOURCE_WRITE_INDEX)) {
        return Err("default-materializer:write-source".into());
    }
    if !statements.get(RETURNED_PATH_INDEX).is_some_and(ok_path) {
        return Err("default-materializer:return-path".into());
    }
    Ok(())
}

fn same_bytes_guard(statement: Option<&syn::Stmt>) -> bool {
    let Some(syn::Stmt::Expr(syn::Expr::If(branch), _)) = statement else {
        return false;
    };
    let syn::Expr::Try(condition) = branch.cond.as_ref() else {
        return false;
    };
    let Some(existing) = call(&condition.expr) else {
        return false;
    };
    path_name_from_expr(&existing.func) == "Self::exists_with_same_bytes"
        && existing.args.len() == 1
        && reference_to_path(&existing.args[0])
        && branch.else_branch.is_none()
        && branch.then_branch.stmts.len() == 1
        && super::syntax::return_call(branch.then_branch.stmts.first(), "Ok", &["path"])
}

fn parent_binding(statement: Option<&syn::Stmt>) -> bool {
    let Some(syn::Stmt::Local(local)) = statement else {
        return false;
    };
    let Some(init) = &local.init else {
        return false;
    };
    super::syntax::is_some_binding(&local.pat, "parent")
        && matches!(&*init.expr, syn::Expr::MethodCall(call) if call.method == "parent" && call.args.is_empty() && ident(&call.receiver, "path"))
        && init
            .diverge
            .as_ref()
            .is_some_and(|(_, expr)| super::syntax::return_error(expr, NO_PARENT))
}

fn write_call(statement: Option<&syn::Stmt>) -> bool {
    let Some(syn::Stmt::Expr(syn::Expr::Try(try_expr), _)) = statement else {
        return false;
    };
    let syn::Expr::MethodCall(map) = try_expr.expr.as_ref() else {
        return false;
    };
    let Some(write) = call(&map.receiver) else {
        return false;
    };
    map.method == "map_err"
        && map.args.len() == 1
        && ident(&map.args[0], "runtime_asset_error")
        && path_name_from_expr(&write.func) == "std::fs::write"
        && write.args.len() == 2
        && reference_to_path(&write.args[0])
        && call_expr(&write.args[1], "MathJaxRuntimeScripts::runtime_source", &[])
}

fn reference_to_path(expression: &syn::Expr) -> bool {
    matches!(expression, syn::Expr::Reference(reference) if ident(&reference.expr, "path"))
}

fn fs_call(statement: Option<&syn::Stmt>, name: &str, arguments: &[&str]) -> bool {
    let Some(syn::Stmt::Expr(syn::Expr::Try(try_expr), _)) = statement else {
        return false;
    };
    let syn::Expr::MethodCall(map) = try_expr.expr.as_ref() else {
        return false;
    };
    map.method == "map_err"
        && map.args.len() == 1
        && ident(&map.args[0], "runtime_asset_error")
        && call_expr(&map.receiver, name, arguments)
}

pub(super) fn call_expr(expression: &syn::Expr, name: &str, arguments: &[&str]) -> bool {
    let Some(actual) = call(expression) else {
        return false;
    };
    path_name_from_expr(&actual.func) == name
        && actual.args.len() == arguments.len()
        && actual.args.iter().zip(arguments).all(|(arg, expected)| {
            if *expected == "MathJaxRuntimeScripts::runtime_source" {
                call_expr(arg, expected, &[])
            } else {
                path_name_from_expr(arg) == *expected
            }
        })
}

fn ok_path(statement: &syn::Stmt) -> bool {
    let syn::Stmt::Expr(expression, _) = statement else {
        return false;
    };
    call_expr(expression, "Ok", &["path"])
}
