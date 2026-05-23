use katana_render_runtime::DiagramKind;

const MIN_MARKDOWN_FENCE_MARKERS: usize = 3;

pub(crate) struct DiagramSourceOps;

impl DiagramSourceOps {
    pub(crate) fn prepare(kind: DiagramKind, source: String) -> String {
        match kind {
            DiagramKind::Mermaid => MermaidMarkdownOps::extract(source),
            DiagramKind::Drawio => source,
            DiagramKind::PlantUml => PlantUmlMarkdownOps::extract(source),
            DiagramKind::MathJax => source,
        }
    }
}

pub(crate) struct MermaidMarkdownOps;

impl MermaidMarkdownOps {
    pub(crate) fn extract(source: String) -> String {
        Self::extract_for_languages(source, &["mermaid", "zenuml"])
    }

    pub(crate) fn extract_for_languages(source: String, languages: &[&'static str]) -> String {
        let mut lines: Vec<&str> = Vec::new();
        let mut block_lang: Option<&'static str> = None;
        for line in source.lines() {
            if block_lang.is_none() {
                block_lang = Self::starts_block(line, languages);
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

    fn starts_block(line: &str, languages: &[&'static str]) -> Option<&'static str> {
        let trimmed = line.trim();
        Self::language_of(trimmed, b'`', languages)
            .or_else(|| Self::language_of(trimmed, b'~', languages))
    }

    fn language_of(line: &str, marker: u8, languages: &[&'static str]) -> Option<&'static str> {
        let bytes = line.as_bytes();
        let marker_count = bytes.iter().take_while(|it| **it == marker).count();
        if marker_count < MIN_MARKDOWN_FENCE_MARKERS {
            return None;
        }
        let language = line[marker_count..].split_whitespace().next()?;
        languages.iter().copied().find(|it| *it == language)
    }

    fn ends_block(line: &str) -> bool {
        matches!(line.trim(), "```" | "~~~")
    }
}

struct PlantUmlMarkdownOps;

impl PlantUmlMarkdownOps {
    fn extract(source: String) -> String {
        MermaidMarkdownOps::extract_for_languages(source, &["plantuml"])
    }
}
