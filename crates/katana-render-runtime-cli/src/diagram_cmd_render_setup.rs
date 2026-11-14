use super::DiagramCommand;
use super::render_attestation::{AttestationPaths, RenderAttestation};
use super::render_input::DiagramRenderRequest;
use crate::commands::DiagramInputFormat;
use crate::diagram_source::DiagramSourceOps;
use katana_render_runtime::{DiagramKind, Renderer, RuntimePathResolver};
use std::path::PathBuf;

impl DiagramCommand {
    pub(super) fn prepare_runtime(
        &self,
        request: &DiagramRenderRequest,
    ) -> anyhow::Result<(PathBuf, Box<dyn Renderer>)> {
        Self::validate_runtime_options(
            self.kind,
            request.runtime.as_ref(),
            request.cache_dir.as_ref(),
        )?;
        let (path, renderer) = RuntimePathResolver::resolve_renderer(
            self.kind,
            request.runtime.clone(),
            request.cache_dir.clone(),
        )?;
        if request.source_options.render_attestation.is_none() {
            return Ok((path, renderer));
        }
        self.prepare_attested_runtime(request, path)
    }

    fn prepare_attested_runtime(
        &self,
        request: &DiagramRenderRequest,
        path: PathBuf,
    ) -> anyhow::Result<(PathBuf, Box<dyn Renderer>)> {
        let path = if self.kind == DiagramKind::Mermaid
            && request.runtime.is_none()
            && std::env::var_os("MERMAID_JS").is_none()
        {
            std::fs::canonicalize(path)?
        } else {
            path
        };
        RuntimePathResolver::resolve_renderer(self.kind, Some(path), None).map_err(Into::into)
    }

    pub(super) fn attestation(
        request: &DiagramRenderRequest,
        runtime: &std::path::Path,
        source: &str,
    ) -> anyhow::Result<Option<RenderAttestation>> {
        request
            .source_options
            .render_attestation
            .as_ref()
            .map(|receipt| {
                RenderAttestation::begin(
                    AttestationPaths {
                        input: &request.input_path,
                        output: request.output_path.as_deref(),
                        runtime,
                        receipt,
                    },
                    request.source_options.attempt_token.as_deref(),
                    request.source_options.input_format,
                    source.as_bytes(),
                )
            })
            .transpose()
    }

    pub(super) fn prepare_source(
        kind: DiagramKind,
        source: String,
        format: DiagramInputFormat,
    ) -> String {
        match format {
            DiagramInputFormat::Auto => DiagramSourceOps::prepare(kind, source),
            DiagramInputFormat::DiagramSource => source,
        }
    }

    pub(super) fn validate_input_options(
        &self,
        request: &DiagramRenderRequest,
    ) -> anyhow::Result<()> {
        request.validate_input_options(self.kind)
    }

    pub(super) fn vendor_config(
        kind: DiagramKind,
        request: &DiagramRenderRequest,
    ) -> anyhow::Result<serde_json::Value> {
        request.vendor_config(kind)
    }

    pub(super) fn validate_runtime_options(
        kind: DiagramKind,
        runtime: Option<&PathBuf>,
        cache_dir: Option<&PathBuf>,
    ) -> anyhow::Result<()> {
        super::render_input::RenderInputFactory::validate_runtime_options(kind, runtime, cache_dir)
    }
}
