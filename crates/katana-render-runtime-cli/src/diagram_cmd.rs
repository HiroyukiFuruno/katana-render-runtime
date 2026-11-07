use crate::commands::{DiagramAction, DiagramInputFormat};
#[cfg(test)]
use crate::diagram_source::DiagramSourceOps;
#[cfg(test)]
pub(crate) use crate::diagram_source::MermaidMarkdownOps;
use crate::file_ops::FileOps;
use crate::reference_cmd::ReferenceCommand;
#[cfg(test)]
use katana_render_runtime::Renderer;
use katana_render_runtime::{DiagramKind, RenderOutput};
#[cfg(test)]
use katana_render_runtime::{DrawioRenderer, MathJaxRenderer, MermaidRenderer, PlantUmlRenderer};
use render_input::{DiagramRenderRequest, RenderInputFactory};
use std::path::PathBuf;

#[path = "render_attestation.rs"]
mod render_attestation;
mod render_input;
#[path = "diagram_cmd_render_setup.rs"]
mod render_setup;

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
        let (runtime_path, renderer) = self.prepare_runtime(&request)?;
        let source = FileOps::read_to_string(&request.input_path)?;
        let mut attestation = Self::attestation(&request, &runtime_path, &source)?;
        let vendor_config = Self::vendor_config(self.kind, &request)?;
        let source = Self::prepare_source(self.kind, source, request.source_options.input_format);
        let input = RenderInputFactory::create(self.kind, source, vendor_config);
        if let Some(attestation) = &attestation {
            attestation.capture_input(&input)?;
        }
        let output = match &mut attestation {
            Some(attestation) => attestation.capture_render(&input.source, || {
                renderer.render(&input).map_err(anyhow::Error::from)
            })?,
            None => renderer.render(&input).map_err(anyhow::Error::from)?,
        };
        match attestation {
            Some(attestation) => attestation.complete(&input, &output),
            None => Self::write_render_output(request.output_path, &output),
        }
    }

    #[cfg(test)]
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

#[cfg(test)]
#[path = "diagram_cmd_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "diagram_cmd_output_tests.rs"]
mod output_tests;
