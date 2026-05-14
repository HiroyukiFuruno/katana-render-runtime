use crate::commands::DiagramAction;
use crate::file_ops::FileOps;
use crate::reference_cmd::ReferenceCommand;
use katana_diagram_renderer::{
    DiagramKind, DrawioRenderer, MermaidRenderer, RenderConfig, RenderContext, RenderInput,
    RenderPolicy, Renderer, RuntimePathResolver,
};
use std::path::PathBuf;

const MIN_MARKDOWN_FENCE_MARKERS: usize = 3;

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
        let mut lines: Vec<&str> = Vec::new();
        let mut block_lang: Option<&'static str> = None;
        for line in source.lines() {
            if block_lang.is_none() {
                block_lang = Self::starts_block(line);
                continue;
            }
            if Self::ends_block(line) {
                let content = lines.join("\n");
                return if block_lang == Some("zenuml") {
                    format!("zenuml\n{content}")
                } else {
                    content
                };
            }
            lines.push(line);
        }
        source
    }

    fn starts_block(line: &str) -> Option<&'static str> {
        let trimmed = line.trim();
        Self::language_of(trimmed, b'`').or_else(|| Self::language_of(trimmed, b'~'))
    }

    fn language_of(line: &str, marker: u8) -> Option<&'static str> {
        let bytes = line.as_bytes();
        let marker_count = bytes.iter().take_while(|it| **it == marker).count();
        if marker_count < MIN_MARKDOWN_FENCE_MARKERS {
            return None;
        }
        match line[marker_count..].split_whitespace().next() {
            Some("mermaid") => Some("mermaid"),
            Some("zenuml") => Some("zenuml"),
            _ => None,
        }
    }

    fn ends_block(line: &str) -> bool {
        matches!(line.trim(), "```" | "~~~")
    }
}

#[cfg(test)]
#[path = "diagram_cmd_tests.rs"]
mod tests;
