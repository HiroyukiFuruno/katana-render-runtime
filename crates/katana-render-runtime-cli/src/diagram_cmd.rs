use crate::commands::{DiagramAction, DiagramInputFormat, DiagramSourceOptions, ThemeModeArg};
use crate::diagram_source::DiagramSourceOps;
#[cfg(test)]
pub(crate) use crate::diagram_source::MermaidMarkdownOps;
use crate::file_ops::FileOps;
use crate::reference_cmd::ReferenceCommand;
use katana_render_runtime::{
    DiagramKind, DrawioRenderer, MathJaxRenderer, MermaidRenderer, PlantUmlRenderer, RenderInput,
    RenderOutput, Renderer, RuntimePathResolver,
};
use render_input::RenderInputFactory;
use std::path::PathBuf;

#[path = "render_attestation.rs"]
mod render_attestation;
mod render_input;
use render_attestation::{AttestationPaths, RenderAttestation};

pub(crate) struct DiagramCommand {
    kind: DiagramKind,
}

impl DiagramCommand {
    pub(crate) fn new(kind: DiagramKind) -> Self {
        Self { kind }
    }

    pub(crate) fn run(self, action: DiagramAction) -> anyhow::Result<()> {
        match action {
            DiagramAction::Render {
                input,
                output,
                runtime,
                theme,
                theme_from,
                theme_mode,
                cache_dir,
                source_options,
            } => self.render(DiagramRenderRequest {
                input_path: input,
                output_path: output,
                runtime,
                theme,
                theme_from,
                theme_mode,
                cache_dir,
                source_options,
            }),
            DiagramAction::ReferenceUpdate { fixtures } => self.update_reference(fixtures),
            DiagramAction::Compare {
                fixtures,
                min_score,
            } => ReferenceCommand::compare(self.kind, fixtures, min_score),
            DiagramAction::Bench { fixtures } => ReferenceCommand::bench(self.kind, fixtures),
        }
    }

    fn update_reference(self, fixtures: PathBuf) -> anyhow::Result<()> {
        ReferenceCommand::update(self.kind, fixtures)
    }

    fn render(self, request: DiagramRenderRequest) -> anyhow::Result<()> {
        self.validate_input_options(&request)?;
        let runtime_path = self.resolve_runtime_path(&request)?;
        let source = Self::read_source(&request.input_path)?;
        let attestation = Self::attestation(&request, &runtime_path, &source)?;
        let vendor_config = Self::vendor_config(self.kind, &request)?;
        let source = Self::prepare_source(self.kind, source, request.source_options.input_format);
        let input = RenderInputFactory::create(self.kind, source, vendor_config);
        if let Some(attestation) = &attestation {
            attestation.capture_input(&input)?;
        }
        let output = self.render_with_runtime(runtime_path, &input)?;
        match attestation {
            Some(attestation) => attestation.complete(&input, &output),
            None => Self::write_render_output(request.output_path, &output),
        }
    }

    fn attestation(
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

    fn prepare_source(kind: DiagramKind, source: String, format: DiagramInputFormat) -> String {
        match format {
            DiagramInputFormat::Auto => DiagramSourceOps::prepare(kind, source),
            DiagramInputFormat::DiagramSource => source,
        }
    }

    fn validate_input_options(&self, request: &DiagramRenderRequest) -> anyhow::Result<()> {
        if self.kind != DiagramKind::Mermaid
            && (request.source_options.input_format != DiagramInputFormat::Auto
                || request.source_options.render_attestation.is_some()
                || request.source_options.attempt_token.is_some())
        {
            anyhow::bail!("--input-format and render attestation are supported only for mermaid");
        }
        if request.source_options.render_attestation.is_none()
            && request.source_options.attempt_token.is_some()
        {
            anyhow::bail!("--attempt-token requires --render-attestation");
        }
        Ok(())
    }

    fn resolve_runtime_path(&self, request: &DiagramRenderRequest) -> anyhow::Result<PathBuf> {
        Self::validate_runtime_options(
            self.kind,
            request.runtime.as_ref(),
            request.cache_dir.as_ref(),
        )?;
        let runtime_path = RuntimePathResolver::resolve_with_plantuml_cache_dir(
            self.kind,
            request.runtime.clone(),
            request.cache_dir.clone(),
        )?;
        Ok(runtime_path)
    }

    fn read_source(input_path: &PathBuf) -> anyhow::Result<String> {
        FileOps::read_to_string(input_path)
    }

    fn vendor_config(
        kind: DiagramKind,
        request: &DiagramRenderRequest,
    ) -> anyhow::Result<serde_json::Value> {
        RenderInputFactory::vendor_config(
            kind,
            request.theme.clone(),
            request.theme_from.clone(),
            request.theme_mode,
            request.cache_dir.clone(),
        )
    }

    fn render_with_runtime(
        &self,
        runtime_path: PathBuf,
        input: &RenderInput,
    ) -> anyhow::Result<RenderOutput> {
        Ok(self.renderer(runtime_path).render(input)?)
    }

    fn validate_runtime_options(
        kind: DiagramKind,
        runtime: Option<&PathBuf>,
        cache_dir: Option<&PathBuf>,
    ) -> anyhow::Result<()> {
        if kind == DiagramKind::PlantUml && runtime.is_some() && cache_dir.is_some() {
            anyhow::bail!("--runtime and --cache-dir cannot be used together for plantuml");
        }
        Ok(())
    }

    fn renderer(&self, runtime_path: PathBuf) -> Box<dyn Renderer> {
        match self.kind {
            DiagramKind::Mermaid => Box::new(MermaidRenderer::with_runtime_path(runtime_path)),
            DiagramKind::Drawio => Box::new(DrawioRenderer::with_runtime_path(runtime_path)),
            DiagramKind::PlantUml => Box::new(PlantUmlRenderer::with_runtime_path(runtime_path)),
            DiagramKind::MathJax => Box::new(MathJaxRenderer::with_runtime_path(runtime_path)),
        }
    }

    fn write_render_output(
        output_path: Option<PathBuf>,
        output: &RenderOutput,
    ) -> anyhow::Result<()> {
        for warning in &output.diagnostics.warnings {
            eprintln!("{warning}");
        }
        match output_path {
            Some(path) => FileOps::write(&path, output.svg.as_bytes()),
            None => {
                print!("{}", output.svg);
                Ok(())
            }
        }
    }
}

struct DiagramRenderRequest {
    input_path: PathBuf,
    output_path: Option<PathBuf>,
    runtime: Option<PathBuf>,
    theme: Option<String>,
    theme_from: Option<String>,
    theme_mode: Option<ThemeModeArg>,
    cache_dir: Option<PathBuf>,
    source_options: DiagramSourceOptions,
}

#[cfg(test)]
#[path = "diagram_cmd_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "diagram_cmd_output_tests.rs"]
mod output_tests;
