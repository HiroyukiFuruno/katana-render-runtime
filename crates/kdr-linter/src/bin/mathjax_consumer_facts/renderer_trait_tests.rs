use super::{
    require_backend_runtime_path, require_render_dispatch, require_render_kind_guard,
    require_render_runtime,
};

const PREFIX: &str = "if input.kind != DiagramKind::MathJax { return Err(RenderError::UnsupportedKind); } let runtime = RuntimeDescriptor::mathjax();";
const DISPATCH: &str = "match self.render_block(input) { Ok(svg) => RenderOutputFactory::from_diagram_result(input, crate::markdown::DiagramResult::Ok(svg), runtime), Err(warning) => Ok(RenderOutputFactory::from_raw_string(input, input.source.clone(), warning, runtime)), }";

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
        let block = syn::parse_str::<syn::Block>(&format!(
            "{{ {PREFIX} match self.render_block(input) {{ {arms} }} }}"
        ))
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
    let non_call = syn::parse_str::<syn::Block>(
        "{ let block = DiagramBlock { kind: crate::markdown::DiagramKind::MathJax, source: input.source.clone() }; let preset = DiagramColorPreset::for_render_input(&input); let value = 1; }",
    )?;
    assert_eq!(
        require_backend_runtime_path(&non_call).err().as_deref(),
        Some("backend-runtime-path:call")
    );

    let non_call_runtime = syn::parse_str::<syn::Block>(
        "{ let block = DiagramBlock { kind: crate::markdown::DiagramKind::MathJax, source: input.source.clone() }; let preset = DiagramColorPreset::for_render_input(&input); not_a_call; }",
    )?;
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
            "{ let first = 1; first; let third = 3; }"
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
