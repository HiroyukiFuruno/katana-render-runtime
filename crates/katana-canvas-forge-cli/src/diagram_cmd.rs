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
        let trimmed = line.trim();
        Self::starts_block_with(trimmed, b'`') || Self::starts_block_with(trimmed, b'~')
    }

    fn starts_block_with(line: &str, marker: u8) -> bool {
        let bytes = line.as_bytes();
        let marker_count = bytes.iter().take_while(|it| **it == marker).count();
        if marker_count < 3 {
            return false;
        }
        line[marker_count..]
            .split_whitespace()
            .next()
            .is_some_and(|language| language == "mermaid")
    }

    fn ends_block(line: &str) -> bool {
        matches!(line.trim(), "```" | "~~~")
    }
}

#[cfg(test)]
#[path = "diagram_cmd_tests.rs"]
mod tests;
