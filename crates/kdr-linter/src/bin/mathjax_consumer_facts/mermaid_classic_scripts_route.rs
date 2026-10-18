use super::super::consumer::require_direct_call;
use super::super::source::{Source, local, method, record_fact};
use super::{CALLER_INPUT_INDEX, INPUTS, plain_local, scripts, tail};
use serde_json::Value;
use syn::spanned::Spanned;

type RequestBinding = (usize, &'static str, &'static str, &'static [&'static str]);

const REQUEST_ROUTE: &str = "mermaid-request-route";
const REQUEST_ROUTE_STATEMENTS: usize = 6;
const REQUEST_BUNDLE_STATEMENT_INDEX: usize = 1;
const REQUEST_SCRIPT_BUILD_STATEMENT_INDEX: usize = 3;
const REQUEST_RENDER_STATEMENT_INDEX: usize = 4;
const REQUEST_BINDINGS: &[RequestBinding] = &[
    (
        0,
        "request",
        "MermaidRenderRequest::new",
        &["source", "preset"],
    ),
    (
        REQUEST_BUNDLE_STATEMENT_INDEX,
        "bundle",
        "read_mermaid_bundle",
        &["mermaid_js"],
    ),
    (
        REQUEST_SCRIPT_BUILD_STATEMENT_INDEX,
        "scripts",
        "MermaidRuntimeScripts::build",
        &["bundle", "request_json", "request.diagram_type"],
    ),
    (
        REQUEST_RENDER_STATEMENT_INDEX,
        "svg",
        "DiagramV8Runtime::render",
        &["scripts"],
    ),
];

pub(super) fn inspect_caller(source: &Source, facts: &mut Vec<Value>) -> Result<(), String> {
    let render = method(&source.ast, "MermaidJsRuntimeOps", "render")?;
    scripts::parameters(&render.sig, &["source", "mermaid_js", "preset"])?;
    let projected = super::super::v8_route::capture::caller(&render.block)?;
    let block = &projected;
    if block.stmts.len() != REQUEST_ROUTE_STATEMENTS {
        return Err("mermaid-request-route:statement-count".into());
    }
    for &(index, binding, callee, args) in REQUEST_BINDINGS {
        plain_local(block, index, binding)?;
        require_direct_call(local(block, index, binding)?, callee, args, REQUEST_ROUTE)?;
    }
    plain_local(block, 2, "request_json")?;
    super::super::call_binding::require_request_json(local(block, 2, "request_json")?)?;
    require_direct_call(tail(block)?, "rendered_svg", &["svg"], REQUEST_ROUTE)?;
    record_fact(
        facts,
        "mermaid-request-build-render-route",
        INPUTS[CALLER_INPUT_INDEX],
        render.span(),
    );
    Ok(())
}
