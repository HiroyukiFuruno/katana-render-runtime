use super::source::{call, ident};

pub(super) const MATHJAX_RENDER_OUTPUT_ARGUMENT_COUNT: usize = 4;
const RENDER_RESULT_ARGUMENT_COUNT: usize = 3;
const RENDER_RESULT_RUNTIME_INDEX: usize = 2;
const WARNING_TEXT_INDEX: usize = 2;
const WARNING_RUNTIME_INDEX: usize = 3;

pub(super) fn is_some_binding(pattern: &syn::Pat, binding: &str) -> bool {
    matches!(pattern, syn::Pat::TupleStruct(value) if super::source::path_name(&value.path) == "Some" && value.elems.len() == 1 && matches!(value.elems.first(), Some(syn::Pat::Ident(ident)) if ident.ident == binding))
}

pub(super) fn return_call(statement: Option<&syn::Stmt>, callee: &str, args: &[&str]) -> bool {
    let Some(syn::Stmt::Expr(syn::Expr::Return(value), _)) = statement else {
        return false;
    };
    value
        .expr
        .as_ref()
        .is_some_and(|expression| call_expr_path(expression, callee, args))
}

pub(super) fn return_expr_call(expression: &syn::Expr, callee: &str, args: &[&str]) -> bool {
    if let syn::Expr::Block(block) = expression {
        return block.block.stmts.len() == 1
            && return_call(block.block.stmts.first(), callee, args);
    }
    let syn::Expr::Return(value) = expression else {
        return false;
    };
    value
        .expr
        .as_ref()
        .is_some_and(|inner| call_expr_path(inner, callee, args))
}

pub(super) fn return_error(expression: &syn::Expr, message: &str) -> bool {
    if let syn::Expr::Block(block) = expression {
        return block.block.stmts.len() == 1
            && block
                .block
                .stmts
                .first()
                .is_some_and(|statement| return_error_statement(statement, message));
    }
    return_error_statement_expr(expression, message)
}

pub(super) fn return_error_statement(statement: &syn::Stmt, message: &str) -> bool {
    let syn::Stmt::Expr(expression, _) = statement else {
        return false;
    };
    return_error_statement_expr(expression, message)
}

pub(super) fn return_error_statement_expr(expression: &syn::Expr, message: &str) -> bool {
    let syn::Expr::Return(value) = expression else {
        return false;
    };
    let Some(value) = value.expr.as_ref() else {
        return false;
    };
    let Some(actual) = call(value) else {
        return false;
    };
    if super::source::path_name_from_expr(&actual.func) != "Err" || actual.args.len() != 1 {
        return false;
    }
    let syn::Expr::MethodCall(to_string) = &actual.args[0] else {
        return false;
    };
    to_string.method == "to_string"
        && to_string.args.is_empty()
        && matches!(&*to_string.receiver, syn::Expr::Lit(literal) if matches!(&literal.lit, syn::Lit::Str(text) if text.value() == message))
}

pub(super) fn ok_some_path(expression: &syn::Expr) -> bool {
    let Some(ok) = call(expression) else {
        return false;
    };
    if super::source::path_name_from_expr(&ok.func) != "Ok" || ok.args.len() != 1 {
        return false;
    }
    let Some(some) = call(&ok.args[0]) else {
        return false;
    };
    if super::source::path_name_from_expr(&some.func) != "Some" || some.args.len() != 1 {
        return false;
    }
    let Some(path) = call(&some.args[0]) else {
        return false;
    };
    super::source::path_name_from_expr(&path.func) == "PathBuf::from"
        && path.args.len() == 1
        && ident(&path.args[0], "path")
}

pub(super) fn local_or_tail(block: &syn::Block, index: usize) -> Result<&syn::Expr, String> {
    if let Some(syn::Stmt::Expr(expression, _)) = block.stmts.get(index) {
        return Ok(expression);
    }
    Err("route-statement".into())
}

pub(super) fn call_expr_path(expression: &syn::Expr, callee: &str, args: &[&str]) -> bool {
    let Some(actual) = call(expression) else {
        return false;
    };
    super::source::path_name_from_expr(&actual.func) == callee
        && actual.args.len() == args.len()
        && actual
            .args
            .iter()
            .zip(args)
            .all(|(argument, name)| super::source::path_name_from_expr(argument) == *name)
}

pub(super) fn require_success_render(expression: &syn::Expr) -> bool {
    let Some(actual) = call(expression) else {
        return false;
    };
    if super::source::path_name_from_expr(&actual.func)
        != "RenderOutputFactory::from_diagram_result"
        || actual.args.len() != RENDER_RESULT_ARGUMENT_COUNT
        || !ident(&actual.args[0], "input")
        || !ident(&actual.args[RENDER_RESULT_RUNTIME_INDEX], "runtime")
    {
        return false;
    }
    let Some(result) = call(&actual.args[1]) else {
        return false;
    };
    super::source::path_name_from_expr(&result.func) == "crate::markdown::DiagramResult::Ok"
        && result.args.len() == 1
        && ident(&result.args[0], "svg")
}

pub(super) fn require_warning_render(expression: &syn::Expr) -> bool {
    let Some(actual) = call(expression) else {
        return false;
    };
    if super::source::path_name_from_expr(&actual.func) != "RenderOutputFactory::from_raw_string"
        || actual.args.len() != MATHJAX_RENDER_OUTPUT_ARGUMENT_COUNT
        || !ident(&actual.args[0], "input")
        || !ident(&actual.args[WARNING_TEXT_INDEX], "warning")
        || !ident(&actual.args[WARNING_RUNTIME_INDEX], "runtime")
    {
        return false;
    }
    matches!(&actual.args[1], syn::Expr::MethodCall(clone) if clone.method == "clone" && clone.args.is_empty() && super::source::path_name_from_expr(&clone.receiver) == "input.source")
}
