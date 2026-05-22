use super::RenderOutputFactory;
use super::RuntimeDescriptor;
use crate::markdown::DiagramResult;
use crate::renderer::api::{DiagramKind, RenderConfig, RenderContext, RenderInput, RenderPolicy};

#[test]
fn output_factory_maps_svg_dimensions_to_public_output() {
    let input = input();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"></svg>"#;
    let output = RenderOutputFactory::from_diagram_result(
        &input,
        DiagramResult::Ok(svg.to_string()),
        RuntimeDescriptor::mermaid(),
    );

    assert!(matches!(output, Ok(it) if it.width == 20.0 && it.view_box.is_empty()));
}

#[test]
fn output_factory_maps_runtime_checksum_to_public_output() {
    let input = input();
    let runtime = RuntimeDescriptor::mermaid();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"></svg>"#;
    let output = RenderOutputFactory::from_diagram_result(
        &input,
        DiagramResult::Ok(svg.to_string()),
        runtime,
    );

    assert!(matches!(
        output,
        Ok(it) if it.runtime.version == runtime.version
            && it.runtime.checksum.as_deref() == Some(runtime.checksum)
    ));
}

#[test]
fn output_factory_maps_error_and_unexpected_outputs() {
    let input = input();
    let runtime = RuntimeDescriptor::mermaid();
    let runtime_error = RenderOutputFactory::from_diagram_result(
        &input,
        DiagramResult::Err {
            source: input.source.clone(),
            error: "boom".to_string(),
        },
        runtime,
    );
    let unexpected = RenderOutputFactory::from_diagram_result(
        &input,
        DiagramResult::OkPng(vec![1, 2, 3]),
        runtime,
    );

    assert!(runtime_error.is_err());
    assert!(unexpected.is_err());
}

#[test]
fn output_factory_maps_raw_code_fallback_to_diagnostics() {
    let input = input();
    let output = RenderOutputFactory::from_diagram_result(
        &input,
        DiagramResult::RawCode {
            source: "@startuml\n@enduml".to_string(),
            warning: "warning[plantuml-runtime-unavailable]: install JDK".to_string(),
        },
        RuntimeDescriptor::plantuml(),
    );

    assert!(matches!(
        output,
        Ok(it) if it.svg.starts_with("```plantuml")
            && it.diagnostics.warnings[0].contains("plantuml-runtime-unavailable")
    ));
}

#[test]
fn output_factory_maps_not_installed_and_invalid_svg() {
    let input = input();
    let runtime = RuntimeDescriptor::mermaid();
    let not_installed = RenderOutputFactory::from_diagram_result(
        &input,
        DiagramResult::NotInstalled {
            kind: "Mermaid".to_string(),
            download_url: "https://example.com/mermaid.js".to_string(),
            install_path: "missing.js".into(),
        },
        runtime,
    );
    let invalid_svg = RenderOutputFactory::from_diagram_result(
        &input,
        DiagramResult::Ok("<svg>".to_string()),
        runtime,
    );

    assert!(not_installed.is_err());
    assert!(invalid_svg.is_err());
}

fn input() -> RenderInput {
    RenderInput {
        kind: DiagramKind::Mermaid,
        source: "graph TD; A-->B".to_string(),
        config: RenderConfig::default(),
        policy: RenderPolicy::default(),
        context: RenderContext::default(),
    }
}
