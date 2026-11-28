use super::super::source::{call, ident, method};

const LOADED_RENDER_ARGUMENT_COUNT: usize = 4;
const LOADED_RESOURCE_ARGUMENT_INDEX: usize = 3;

pub(super) fn require_source_input(renderer: &syn::File, file: &syn::File) -> Result<(), String> {
    super::super::call_binding::require_runtime_source_reader(file)?;
    if !super::super::materializer::canonical_materializer_impl(renderer, "MathJaxRendererOps") {
        return Err("default-materializer:renderer-owner-context".into());
    }
    let route = method(renderer, "MathJaxRendererOps", "render_mathjax_with_loaded")?;
    let Some(syn::Stmt::Expr(expression, _)) = route.block.stmts.last() else {
        return Err("read-render-route:loaded-call".into());
    };
    let Some(call) = call(expression) else {
        return Err("read-render-route:loaded-call".into());
    };
    if super::super::source::path_name_from_expr(&call.func) != "MathJaxJsRuntimeOps::render_bundle"
        || call.args.len() != LOADED_RENDER_ARGUMENT_COUNT
        || !ident(&call.args[0], "block.source")
        || !matches!(&call.args[1], syn::Expr::MethodCall(value)
            if value.method == "text" && value.args.is_empty() && ident(&value.receiver, "resource"))
        || !ident(&call.args[2], "preset")
        || !ident(&call.args[LOADED_RESOURCE_ARGUMENT_INDEX], "display")
    {
        return Err("read-render-route:loaded-call".into());
    }
    Ok(())
}
