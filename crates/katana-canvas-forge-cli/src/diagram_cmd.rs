use crate::commands::DiagramAction;
use crate::file_ops::FileOps;
use crate::reference_cmd::ReferenceCommand;
use katana_canvas_forge::{
    DiagramKind, DrawioRenderer, MermaidRenderer, RenderConfig, RenderContext, RenderInput,
    RenderPolicy, Renderer, RuntimePathResolver,
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
            } => self.render(input, output, runtime),
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

    fn render(
        self,
        input_path: PathBuf,
        output_path: PathBuf,
        runtime: Option<PathBuf>,
    ) -> anyhow::Result<()> {
        let runtime_path = RuntimePathResolver::resolve(self.kind, runtime)?;
        let source = FileOps::read_to_string(&input_path)?;
        let input =
            RenderInputFactory::create(self.kind, DiagramSourceOps::prepare(self.kind, source));
        let output = self.renderer(runtime_path).render(&input)?;
        FileOps::write(&output_path, output.svg.as_bytes())
    }

    fn renderer(self, runtime_path: PathBuf) -> Box<dyn Renderer> {
        match self.kind {
            DiagramKind::Mermaid => Box::new(MermaidRenderer::with_runtime_path(runtime_path)),
            DiagramKind::Drawio => Box::new(DrawioRenderer::with_runtime_path(runtime_path)),
        }
    }
}

struct RenderInputFactory;

impl RenderInputFactory {
    fn create(kind: DiagramKind, source: String) -> RenderInput {
        RenderInput {
            kind,
            source,
            config: RenderConfig::default(),
            policy: RenderPolicy::default(),
            context: RenderContext::default(),
        }
    }
}

struct DiagramSourceOps;

impl DiagramSourceOps {
    fn prepare(kind: DiagramKind, source: String) -> String {
        match kind {
            DiagramKind::Mermaid => MermaidMarkdownOps::extract(source),
            DiagramKind::Drawio => source,
        }
    }
}

struct MermaidMarkdownOps;

impl MermaidMarkdownOps {
    fn extract(source: String) -> String {
        let mut lines = Vec::new();
        let mut in_block = false;
        for line in source.lines() {
            if Self::starts_block(line) {
                in_block = true;
                continue;
            }
            if in_block && Self::ends_block(line) {
                return lines.join("\n");
            }
            if in_block {
                lines.push(line);
            }
        }
        source
    }

    fn starts_block(line: &str) -> bool {
        matches!(line.trim(), "```mermaid" | "~~~mermaid")
    }

    fn ends_block(line: &str) -> bool {
        matches!(line.trim(), "```" | "~~~")
    }
}

#[cfg(test)]
mod tests {
    use super::{DiagramCommand, DiagramSourceOps, MermaidMarkdownOps, RenderInputFactory};
    use crate::commands::DiagramAction;
    use katana_canvas_forge::DiagramKind;

    #[test]
    fn extracts_mermaid_fence_from_markdown() {
        let source = "# Title\n\n~~~mermaid\ngraph TD; A-->B\n~~~\n".to_string();
        assert_eq!(MermaidMarkdownOps::extract(source), "graph TD; A-->B");
    }

    #[test]
    fn keeps_source_when_mermaid_fence_is_not_closed() {
        let source = "```mermaid\ngraph TD; A-->B".to_string();
        assert_eq!(MermaidMarkdownOps::extract(source.clone()), source);
    }

    #[test]
    fn drawio_source_passes_through() {
        let source = "<mxGraphModel />".to_string();
        assert_eq!(
            DiagramSourceOps::prepare(DiagramKind::Drawio, source.clone()),
            source
        );
    }

    #[test]
    fn render_input_factory_sets_kind_and_source() {
        let input = RenderInputFactory::create(DiagramKind::Mermaid, "graph TD; A".to_string());
        assert_eq!(input.kind, DiagramKind::Mermaid);
        assert_eq!(input.source, "graph TD; A");
    }

    #[test]
    fn mermaid_render_reports_missing_runtime() -> Result<(), Box<dyn std::error::Error>> {
        let input = std::env::temp_dir().join(format!("kcf-cli-mmd-{}.md", std::process::id()));
        let output = std::env::temp_dir().join(format!("kcf-cli-mmd-{}.svg", std::process::id()));
        let runtime = std::env::temp_dir().join("missing-kcf-mermaid-runtime.js");

        std::fs::write(&input, "```mermaid\ngraph TD; A-->B\n```\n")?;
        let result = DiagramCommand::new(DiagramKind::Mermaid).run(DiagramAction::Render {
            input: input.clone(),
            output,
            runtime: Some(runtime),
        });

        assert!(result.is_err());
        std::fs::remove_file(input)?;
        Ok(())
    }

    #[test]
    fn drawio_render_reports_missing_runtime() -> Result<(), Box<dyn std::error::Error>> {
        let input =
            std::env::temp_dir().join(format!("kcf-cli-drawio-{}.drawio", std::process::id()));
        let output =
            std::env::temp_dir().join(format!("kcf-cli-drawio-{}.svg", std::process::id()));
        let runtime = std::env::temp_dir().join("missing-kcf-drawio-runtime.js");

        std::fs::write(&input, "<mxGraphModel />")?;
        let result = DiagramCommand::new(DiagramKind::Drawio).run(DiagramAction::Render {
            input: input.clone(),
            output,
            runtime: Some(runtime),
        });

        assert!(result.is_err());
        std::fs::remove_file(input)?;
        Ok(())
    }
}
