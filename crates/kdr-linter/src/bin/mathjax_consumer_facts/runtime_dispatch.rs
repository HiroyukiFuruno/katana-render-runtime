use super::source::{RUST_INPUTS, Source, call, ident, method, record_fact};
use super::syntax::call_expr_path;
use super::syntax::return_call;
use serde_json::Value;
use std::collections::BTreeMap;
use syn::spanned::Spanned;

#[path = "runtime_resolver_loaded.rs"]
mod loaded_route;

const RUNTIME_RESOLVER_STATEMENTS: usize = 2;
const RUNTIME_PATH_INPUT_INDEX: usize = 3;

pub(super) fn inspect_runtime_resolver(
    sources: &BTreeMap<&str, Source>,
    facts: &mut Vec<Value>,
) -> Result<(), String> {
    let path = RUST_INPUTS[RUNTIME_PATH_INPUT_INDEX];
    let resolver = method(
        &sources[path].ast,
        "RuntimePathResolver",
        "resolve_with_plantuml_cache_dir",
    )?;
    require_resolver_body(&resolver.block)?;
    let loaded = method(
        &sources[path].ast,
        "RuntimePathResolver",
        "resolve_renderer",
    )?;
    loaded_route::require_loaded_resolver(&sources[path].ast, &loaded.block)?;
    record_fact(facts, "runtime-path-resolver", path, resolver.span());
    Ok(())
}

fn require_resolver_body(block: &syn::Block) -> Result<(), String> {
    if block.stmts.len() != RUNTIME_RESOLVER_STATEMENTS {
        return Err("runtime-path-resolver:statement-count".into());
    }
    require_early_runtime_path(block)?;
    let dispatch = runtime_dispatch_expression(block)?;
    require_runtime_dispatch(dispatch)?;
    Ok(())
}

fn require_early_runtime_path(block: &syn::Block) -> Result<(), String> {
    let Some(syn::Stmt::Expr(syn::Expr::If(early), _)) = block.stmts.first() else {
        return Err("runtime-path-resolver:early-return".into());
    };
    let syn::Expr::Let(condition) = early.cond.as_ref() else {
        return Err("runtime-path-resolver:early-return".into());
    };
    if !ident(&condition.expr, "runtime_path")
        || early.else_branch.is_some()
        || early.then_branch.stmts.len() != 1
        || !return_call(early.then_branch.stmts.first(), "Ok", &["path"])
    {
        return Err("runtime-path-resolver:early-return".into());
    }
    Ok(())
}

fn runtime_dispatch_expression(block: &syn::Block) -> Result<&syn::ExprMatch, String> {
    let Some(syn::Stmt::Expr(syn::Expr::MethodCall(map_err), _)) = block.stmts.get(1) else {
        return Err("runtime-path-resolver:dispatch".into());
    };
    if map_err.method != "map_err" {
        return Err("runtime-path-resolver:map-err".into());
    }
    let mut arguments = map_err.args.iter();
    let Some(argument) = arguments.next() else {
        return Err("runtime-path-resolver:map-err".into());
    };
    if arguments.next().is_some() || !ident(argument, "RenderError::RuntimeResolution") {
        return Err("runtime-path-resolver:map-err".into());
    }
    let syn::Expr::Match(dispatch) = map_err.receiver.as_ref() else {
        return Err("runtime-path-resolver:dispatch".into());
    };
    Ok(dispatch)
}
fn require_runtime_dispatch(dispatch: &syn::ExprMatch) -> Result<(), String> {
    if !ident(&dispatch.expr, "kind") {
        return Err("runtime-path-resolver:dispatch-shape".into());
    }
    let [mermaid, drawio, mathjax, plantuml] = dispatch.arms.as_slice() else {
        return Err("runtime-path-resolver:dispatch-shape".into());
    };
    require_standard_dispatches(mermaid, drawio, mathjax)?;
    require_dispatch_plantuml(plantuml)
}

fn require_standard_dispatches(
    mermaid: &syn::Arm,
    drawio: &syn::Arm,
    mathjax: &syn::Arm,
) -> Result<(), String> {
    require_dispatch_call(
        mermaid,
        "DiagramKind::Mermaid",
        "MermaidBinaryOps::resolve_mermaid_js",
    )?;
    require_dispatch_call(
        drawio,
        "DiagramKind::Drawio",
        "DrawioRendererOps::resolve_drawio_js",
    )?;
    require_dispatch_call(
        mathjax,
        "DiagramKind::MathJax",
        "MathJaxRendererOps::resolve_mathjax_js",
    )
}
fn require_dispatch_call(arm: &syn::Arm, pattern: &str, callee: &str) -> Result<(), String> {
    if !matches!(&arm.pat, syn::Pat::Path(value) if super::source::path_name(&value.path) == pattern)
        || !call_expr_path(&arm.body, callee, &[])
    {
        return Err(format!("runtime-path-resolver:arm:{pattern}"));
    }
    Ok(())
}
fn require_dispatch_plantuml(arm: &syn::Arm) -> Result<(), String> {
    if !matches!(&arm.pat, syn::Pat::Path(value) if super::source::path_name(&value.path) == "DiagramKind::PlantUml")
    {
        return Err("runtime-path-resolver:arm:DiagramKind::PlantUml".into());
    }
    let map_or_else = plantuml_map_or_else(arm)?;
    let arguments = map_or_else.args.iter().collect::<Vec<_>>();
    let [default, cached] = arguments.as_slice() else {
        return Err("runtime-path-resolver:plantuml-arm".into());
    };
    require_plantuml_default(default)?;
    require_plantuml_cached(cached)
}

fn plantuml_map_or_else(arm: &syn::Arm) -> Result<&syn::ExprMethodCall, String> {
    let Some(ok) = call(&arm.body) else {
        return Err("runtime-path-resolver:plantuml-arm".into());
    };
    if super::source::path_name_from_expr(&ok.func) != "Ok" || ok.args.len() != 1 {
        return Err("runtime-path-resolver:plantuml-arm".into());
    }
    let syn::Expr::MethodCall(map_or_else) = ok
        .args
        .first()
        .ok_or("runtime-path-resolver:plantuml-arm")?
    else {
        return Err("runtime-path-resolver:plantuml-arm".into());
    };
    if map_or_else.method != "map_or_else" || !ident(&map_or_else.receiver, "plantuml_cache_dir") {
        return Err("runtime-path-resolver:plantuml-map-or-else".into());
    }
    Ok(map_or_else)
}

fn require_plantuml_default(default: &syn::Expr) -> Result<(), String> {
    if super::source::path_name_from_expr(default) != "PlantUmlRendererOps::default_jar_path" {
        return Err("runtime-path-resolver:plantuml-default".into());
    }
    Ok(())
}

fn require_plantuml_cached(cached: &syn::Expr) -> Result<(), String> {
    let syn::Expr::Closure(closure) = cached else {
        return Err("runtime-path-resolver:plantuml-arm".into());
    };
    if closure.inputs.len() != 1
        || !matches!(closure.inputs.first(), Some(syn::Pat::Ident(value)) if value.ident == "path")
    {
        return Err("runtime-path-resolver:plantuml-closure".into());
    }
    let cache_path = plantuml_cache_call(&closure.body)?;
    if super::source::path_name_from_expr(&cache_path.func)
        != "PlantUmlRendererOps::default_jar_path_for_cache_dir"
        || cache_path.args.len() != 1
        || !matches!(&cache_path.args[0], syn::Expr::Reference(value) if ident(&value.expr, "path"))
    {
        return Err("runtime-path-resolver:plantuml-cache".into());
    }
    Ok(())
}

fn plantuml_cache_call(expression: &syn::Expr) -> Result<&syn::ExprCall, String> {
    let expression = match expression {
        syn::Expr::Block(block) if block.block.stmts.len() == 1 => {
            match block.block.stmts.first() {
                Some(syn::Stmt::Expr(expression, _)) => expression,
                _ => return Err("runtime-path-resolver:plantuml-cache".into()),
            }
        }
        expression => expression,
    };
    call(expression).ok_or_else(|| "runtime-path-resolver:plantuml-cache".into())
}

#[cfg(test)]
mod private_ast_tests {
    use super::RUNTIME_PATH_INPUT_INDEX;
    use crate::source::{OPAQUE_INPUTS, RUST_INPUTS};
    use crate::test_support::Fixture;

    const PLANTUML_ARM: &str = concat!(
        "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),\n",
        "            DiagramKind::PlantUml => Ok(plantuml_cache_dir\n",
        "                .map_or_else(PlantUmlRendererOps::default_jar_path, |path| {\n",
        "                    PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)\n",
        "                })),\n",
    );
    const PLANTUML_BOUNDARY_MUTATIONS: &[(&str, &str, &str)] = &[
        (
            PLANTUML_ARM,
            concat!(
                "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),\n",
                "            DiagramKind::PlantUml => Ok(plantuml_cache_dir.map_or_else(PlantUmlRendererOps::default_jar_path)),\n"
            ),
            "runtime-path-resolver:plantuml-arm",
        ),
        (
            PLANTUML_ARM,
            concat!(
                "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),\n",
                "            DiagramKind::PlantUml => Ok(plantuml_cache_dir),\n"
            ),
            "runtime-path-resolver:plantuml-arm",
        ),
        (
            PLANTUML_ARM,
            concat!(
                "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),\n",
                "            DiagramKind::PlantUml => Ok(plantuml_cache_dir.map_or_else(PlantUmlRendererOps::default_jar_path, |path| { let _cache = path; })),\n"
            ),
            "runtime-path-resolver:plantuml-cache",
        ),
        (
            PLANTUML_ARM,
            concat!(
                "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),\n",
                "            DiagramKind::PlantUml => Ok(plantuml_cache_dir.map_or_else(PlantUmlRendererOps::default_jar_path, |path| path)),\n"
            ),
            "runtime-path-resolver:plantuml-cache",
        ),
    ];

    #[test]
    fn plantuml_dispatch_rejects_map_or_else_shape_variants()
    -> Result<(), Box<dyn std::error::Error>> {
        let inputs: Vec<&str> = [RUST_INPUTS, OPAQUE_INPUTS].concat();
        let fixture = Fixture::copy_mathjax_inputs(&inputs)?;
        fixture.reject_unique_mutations(
            RUST_INPUTS[RUNTIME_PATH_INPUT_INDEX],
            PLANTUML_BOUNDARY_MUTATIONS,
        )?;
        Ok(())
    }
}
