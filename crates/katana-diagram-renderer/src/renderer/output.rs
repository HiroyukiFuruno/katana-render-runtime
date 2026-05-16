use super::api::{
    RenderDiagnostics, RenderError, RenderInput, RenderOutput, RendererProfile, RuntimeVersion,
};
use super::fingerprint::CacheFingerprintOps;
use super::metadata::SvgMetadataOps;
use super::runtime::RuntimeDescriptor;
use crate::markdown::DiagramResult;

pub(super) struct RenderOutputFactory;

impl RenderOutputFactory {
    pub(super) fn from_diagram_result(
        input: &RenderInput,
        result: DiagramResult,
        runtime: RuntimeDescriptor,
    ) -> Result<RenderOutput, RenderError> {
        match result {
            DiagramResult::Ok(svg) => Self::from_svg(input, svg, runtime),
            DiagramResult::Err { error, .. } => Err(RenderError::Runtime(error)),
            DiagramResult::NotInstalled {
                kind,
                download_url,
                install_path,
            } => Err(RenderError::NotInstalled {
                kind,
                download_url,
                install_path,
            }),
            _ => Err(RenderError::Runtime(
                "unexpected diagram output".to_string(),
            )),
        }
    }

    fn from_svg(
        input: &RenderInput,
        svg: String,
        runtime: RuntimeDescriptor,
    ) -> Result<RenderOutput, RenderError> {
        let metadata = SvgMetadataOps::parse(&svg)?;
        Ok(RenderOutput {
            cache_fingerprint: CacheFingerprintOps::render(
                input,
                runtime.version,
                runtime.checksum,
            ),
            svg,
            width: metadata.width,
            height: metadata.height,
            view_box: metadata.view_box,
            runtime: Self::runtime(runtime),
            profile: Self::profile(runtime),
            diagnostics: Self::diagnostics(),
        })
    }

    fn runtime(runtime: RuntimeDescriptor) -> RuntimeVersion {
        RuntimeVersion {
            name: runtime.name.to_string(),
            version: runtime.version.to_string(),
            checksum: Some(runtime.checksum.to_string()),
        }
    }

    fn profile(runtime: RuntimeDescriptor) -> RendererProfile {
        RendererProfile {
            id: runtime.profile_id.to_string(),
            description: None,
        }
    }

    fn diagnostics() -> RenderDiagnostics {
        RenderDiagnostics {
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RenderOutputFactory;
    use super::RuntimeDescriptor;
    use crate::markdown::DiagramResult;
    use crate::renderer::api::{
        DiagramKind, RenderConfig, RenderContext, RenderInput, RenderPolicy,
    };

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
        assert!(runtime_error.is_err());

        let unexpected = RenderOutputFactory::from_diagram_result(
            &input,
            DiagramResult::OkPng(vec![1, 2, 3]),
            runtime,
        );
        assert!(unexpected.is_err());
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
        assert!(not_installed.is_err());

        let invalid_svg = RenderOutputFactory::from_diagram_result(
            &input,
            DiagramResult::Ok("<svg>".to_string()),
            runtime,
        );
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
}
