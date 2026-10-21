use super::require_render_dispatch;

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
