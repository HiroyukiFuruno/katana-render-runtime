use super::{
    require_backend_runtime_path, require_render_dispatch, require_render_kind_guard,
    require_render_runtime,
};

const PREFIX: &str = "if input.kind != DiagramKind::MathJax { return Err(RenderError::UnsupportedKind); } let runtime = RuntimeDescriptor::mathjax(); let (result, digest) = self.render_block(input);";
const DISPATCH: &str = "match result { Ok(svg) => RenderOutputFactory::from_diagram_result(input, crate::markdown::DiagramResult::Ok(svg), runtime).map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref())), Err(warning) => { let error_key = format!(\"error:{}:{warning}\", digest.as_deref().unwrap_or(\"\")); Ok(RenderOutputFactory::bind_resource(RenderOutputFactory::from_raw_string(input, input.source.clone(), warning, runtime), Some(&error_key))) }, }";
const RESOURCE: &str = "if block.source.trim().is_empty() { return (Err(\"MathJax source is empty\".to_string()), None); } let resource = match self.read_resource() { Ok(resource) => resource, Err(error) => return (Err(error), None), };";

#[test]
fn render_dispatch_rejects_wrong_arm_counts_with_dispatch_diagnostic() -> Result<(), String> {
    let accepted = syn::parse_str::<syn::Block>(&format!("{{ {PREFIX} {DISPATCH} }}"))
        .map_err(|error| error.to_string())?;
    assert_eq!(require_render_dispatch(&accepted), Ok(()));
    for arms in [
        "",
        "Ok(svg) => RenderOutputFactory::from_diagram_result(input, crate::markdown::DiagramResult::Ok(svg), runtime),",
        "Ok(svg) => RenderOutputFactory::from_diagram_result(input, crate::markdown::DiagramResult::Ok(svg), runtime), Err(warning) => Ok(RenderOutputFactory::from_raw_string(input, input.source.clone(), warning, runtime)), _ => Err(RenderError::UnsupportedKind)",
    ] {
        let block =
            syn::parse_str::<syn::Block>(&format!("{{ {PREFIX} match result {{ {arms} }} }}"))
                .map_err(|error| error.to_string())?;
        assert_eq!(
            require_render_dispatch(&block).err().as_deref(),
            Some("backend-runtime-path:render-dispatch")
        );
    }
    Ok(())
}

#[test]
fn rejects_backend_path_shape_edges() -> Result<(), syn::Error> {
    let non_call = syn::parse_str::<syn::Block>(&format!(
        "{{ let block = DiagramBlock {{ kind: crate::markdown::DiagramKind::MathJax, source: input.source.clone() }}; {RESOURCE} let preset = DiagramColorPreset::for_render_input(&input); let result = 1; (result, resource.into_digest()) }}",
    ))?;
    assert_eq!(
        require_backend_runtime_path(&non_call).err().as_deref(),
        Some("backend-runtime-path:call")
    );

    let non_call_runtime = syn::parse_str::<syn::Block>(&format!(
        "{{ let block = DiagramBlock {{ kind: crate::markdown::DiagramKind::MathJax, source: input.source.clone() }}; {RESOURCE} let preset = DiagramColorPreset::for_render_input(&input); let result = not_a_call; (result, resource.into_digest()) }}",
    ))?;
    assert_eq!(
        require_backend_runtime_path(&non_call_runtime)
            .err()
            .as_deref(),
        Some("backend-runtime-path:call")
    );
    Ok(())
}

#[test]
fn rejects_render_route_shape_edges() -> Result<(), syn::Error> {
    assert_eq!(
        require_render_kind_guard(&syn::parse_str::<syn::Block>("{ let value = 1; }")?)
            .err()
            .as_deref(),
        Some("backend-runtime-path:render-route-statement-count")
    );
    assert_eq!(
        require_render_kind_guard(&syn::parse_str::<syn::Block>(
            "{ let first = 1; first; let third = 3; let fourth = 4; }"
        )?)
        .err()
        .as_deref(),
        Some("backend-runtime-path:render-kind-check")
    );
    Ok(())
}

#[test]
fn rejects_render_runtime_and_dispatch_shapes() -> Result<(), syn::Error> {
    assert_eq!(
        require_render_runtime(&syn::parse_str::<syn::Block>(
            "{ let first = 1; first; let third = 3; }"
        )?)
        .err()
        .as_deref(),
        Some("backend-runtime-path:render-runtime")
    );
    assert_eq!(
        require_render_dispatch(&syn::parse_str::<syn::Block>(
            "{ let first = 1; let second = 2; let third = 3; }"
        )?)
        .err()
        .as_deref(),
        Some("backend-runtime-path:render-dispatch")
    );
    Ok(())
}

#[test]
fn output_binding_guards_reject_malformed_expressions() -> Result<(), syn::Error> {
    for source in ["other", "other.map()"] {
        let expression = syn::parse_str(source)?;
        assert!(!super::resource_route::OutputGuard::require_bound_success(
            &expression
        ));
    }
    let key = r#"let error_key = format!("error:{}:{warning}", digest.as_deref().unwrap_or("")); "#;
    for body in [
        "let other = 0; Ok(())".to_string(),
        format!("{key} let other = 0;"),
        format!("{key} Err(())"),
        format!("{key} Ok(other)"),
    ] {
        let expression = syn::parse_str(&format!("{{ {body} }}"))?;
        assert!(!super::resource_route::OutputGuard::require_bound_warning(
            &expression
        ));
    }
    Ok(())
}
