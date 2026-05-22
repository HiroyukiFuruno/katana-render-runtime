use katana_diagram_renderer::{
    MathJaxRenderer, RenderConfig, RenderContext, RenderError, RenderInput, RenderKind,
    RenderPolicy, Renderer,
};

#[test]
fn wrapper_reexports_runtime_public_api() {
    let renderer: Box<dyn Renderer> = Box::new(MathJaxRenderer::with_runtime_path(
        "missing-mathjax.js".into(),
    ));
    let input = RenderInput {
        kind: RenderKind::Mermaid,
        source: "x".to_string(),
        config: RenderConfig::default(),
        policy: RenderPolicy::default(),
        context: RenderContext::default(),
    };

    assert!(matches!(
        renderer.render(&input),
        Err(RenderError::UnsupportedKind)
    ));
}
