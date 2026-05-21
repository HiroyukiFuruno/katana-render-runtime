use crate::commands::{DiagramAction, ThemeModeArg};
use crate::diagram_source::DiagramSourceOps;
#[cfg(test)]
pub(crate) use crate::diagram_source::MermaidMarkdownOps;
use crate::file_ops::FileOps;
use crate::reference_cmd::ReferenceCommand;
use katana_diagram_renderer::{
    DiagramKind, DrawioRenderer, MermaidRenderer, PlantUmlRenderer, RenderConfig, RenderContext,
    RenderInput, RenderOutput, RenderPolicy, Renderer, RuntimePathResolver,
};
use std::path::PathBuf;

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
            } => self.render(DiagramRenderRequest {
                input_path: input,
                output_path: output,
                runtime,
                theme,
                theme_from,
                theme_mode,
                cache_dir,
            }),
            DiagramAction::ReferenceUpdate { fixtures } => {
                ReferenceCommand::update(self.kind, fixtures)
            }
            DiagramAction::Compare {
                fixtures,
                min_score,
            } => ReferenceCommand::compare(self.kind, fixtures, min_score),
            DiagramAction::Bench { fixtures } => ReferenceCommand::bench(self.kind, fixtures),
        }
    }

    fn render(self, request: DiagramRenderRequest) -> anyhow::Result<()> {
        Self::validate_runtime_options(
            self.kind,
            request.runtime.as_ref(),
            request.cache_dir.as_ref(),
        )?;
        let runtime_path = RuntimePathResolver::resolve_with_plantuml_cache_dir(
            self.kind,
            request.runtime,
            request.cache_dir.clone(),
        )?;
        let source = FileOps::read_to_string(&request.input_path)?;
        let input = RenderInputFactory::create(
            self.kind,
            DiagramSourceOps::prepare(self.kind, source),
            RenderInputFactory::vendor_config(
                self.kind,
                request.theme,
                request.theme_from,
                request.theme_mode,
                request.cache_dir,
            )?,
        );
        let output = self.renderer(runtime_path).render(&input)?;
        Self::write_render_output(request.output_path, &output)
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
}

struct RenderInputFactory;

impl RenderInputFactory {
    fn create(kind: DiagramKind, source: String, vendor_config: serde_json::Value) -> RenderInput {
        RenderInput {
            kind,
            source,
            config: RenderConfig { vendor_config },
            policy: RenderPolicy::default(),
            context: RenderContext::default(),
        }
    }

    fn vendor_config(
        kind: DiagramKind,
        theme: Option<String>,
        theme_from: Option<String>,
        theme_mode: Option<ThemeModeArg>,
        cache_dir: Option<PathBuf>,
    ) -> anyhow::Result<serde_json::Value> {
        if theme.is_none() && theme_from.is_none() && theme_mode.is_none() && cache_dir.is_none() {
            return Ok(serde_json::Value::Null);
        }
        if kind != DiagramKind::PlantUml {
            anyhow::bail!(
                "--theme, --theme-from, --theme-mode, and --cache-dir are currently supported only for plantuml"
            );
        }
        Ok(serde_json::json!({
            "plantuml_theme": theme.unwrap_or_default(),
            "plantuml_theme_from": theme_from.unwrap_or_default(),
            "plantuml_theme_mode": theme_mode.map_or("", ThemeModeArg::as_str),
            "plantuml_cache_dir": cache_dir.map_or_else(String::new, |it| it.display().to_string()),
        }))
    }
}

#[cfg(test)]
#[path = "diagram_cmd_tests.rs"]
mod tests;
