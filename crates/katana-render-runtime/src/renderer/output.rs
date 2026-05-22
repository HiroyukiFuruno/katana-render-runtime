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
            DiagramResult::RawCode { source, warning } => {
                Ok(Self::from_raw_code(input, source, warning, runtime))
            }
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

    pub(super) fn from_raw_string(
        input: &RenderInput,
        source: String,
        error: String,
        runtime: RuntimeDescriptor,
    ) -> RenderOutput {
        tracing::warn!("{}", error);
        RenderOutput {
            cache_fingerprint: CacheFingerprintOps::render(
                input,
                runtime.version,
                runtime.checksum,
            ),
            svg: source,
            width: 0.0,
            height: 0.0,
            view_box: String::new(),
            runtime: Self::runtime(runtime),
            profile: Self::profile(runtime),
            diagnostics: RenderDiagnostics {
                warnings: Vec::new(),
                errors: vec![error],
            },
        }
    }

    fn from_raw_code(
        input: &RenderInput,
        source: String,
        warning: String,
        runtime: RuntimeDescriptor,
    ) -> RenderOutput {
        tracing::warn!("{}", warning);
        RenderOutput {
            cache_fingerprint: CacheFingerprintOps::render(
                input,
                runtime.version,
                runtime.checksum,
            ),
            svg: Self::raw_code_block(&source),
            width: 0.0,
            height: 0.0,
            view_box: String::new(),
            runtime: Self::runtime(runtime),
            profile: Self::profile(runtime),
            diagnostics: RenderDiagnostics {
                warnings: vec![warning],
                errors: Vec::new(),
            },
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

    fn raw_code_block(source: &str) -> String {
        format!("```plantuml\n{}\n```", source.trim_end())
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
