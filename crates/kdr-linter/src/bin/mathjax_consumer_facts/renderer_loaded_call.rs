use super::super::source::{call, ident};
use super::super::syntax::call_expr_path;

const RESOURCE_ARGUMENT_INDEX: usize = 3;

pub(super) fn require_runtime_path_call(expression: &syn::Expr) -> Result<(), String> {
    let Some(actual) = call(expression) else {
        return Err("backend-runtime-path:call".into());
    };
    if super::super::source::path_name_from_expr(&actual.func)
        != "MathJaxRendererOps::render_mathjax_with_loaded"
        || actual.args.len() != super::super::syntax::MATHJAX_RENDER_OUTPUT_ARGUMENT_COUNT
        || !ident(&actual.args[0], "block")
        || !ident(&actual.args[1], "preset")
        || !ident(&actual.args[RESOURCE_ARGUMENT_INDEX], "resource")
        || !call_expr_path(&actual.args[2], "MathJaxRenderConfig::display", &["input"])
    {
        return Err("backend-runtime-path:arguments".into());
    }
    Ok(())
}
