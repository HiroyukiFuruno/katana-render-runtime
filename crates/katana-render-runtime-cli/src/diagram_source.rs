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

struct MarkdownFence {
    language: Option<&'static str>,
    marker: u8,
    length: usize,
}

impl MermaidMarkdownOps {
    pub(crate) fn extract(source: String) -> String {
        Self::extract_for_languages(source, &["mermaid", "zenuml"])
    }

    pub(crate) fn extract_for_languages(source: String, languages: &[&'static str]) -> String {
        let mut lines: Vec<&str> = Vec::new();
        let mut fence: Option<MarkdownFence> = None;
        for line in source.lines() {
            let Some(open) = &fence else {
                fence = Self::starts_block(line, languages);
                continue;
            };
            if Self::ends_block(line, open.marker, open.length) {
                if let Some(language) = open.language {
                    return Self::format_block(language, &lines);
                }
                fence = None;
            } else if open.language.is_some() {
                lines.push(line);
            }
        }
        source
    }

    fn format_block(language: &str, lines: &[&str]) -> String {
        let content = lines.join("\n");
        if language == "zenuml" {
            format!("zenuml\n{content}")
        } else {
            content
        }
    }

    fn starts_block(line: &str, languages: &[&'static str]) -> Option<MarkdownFence> {
        let line = line.trim();
        let bytes = line.as_bytes();
        let marker = *bytes.first()?;
        if !matches!(marker, b'`' | b'~') {
            return None;
        }
        let length = bytes.iter().take_while(|it| **it == marker).count();
        if length < MIN_MARKDOWN_FENCE_MARKERS {
            return None;
        }
        let info = &line[length..];
        let tag = info.split_whitespace().next();
        Some(MarkdownFence {
            language: languages
                .iter()
                .copied()
                .find(|language| Some(*language) == tag),
            marker,
            length,
        })
    }

    fn ends_block(line: &str, marker: u8, min_count: usize) -> bool {
        let bytes = line.trim().as_bytes();
        bytes.len() >= min_count && bytes.iter().all(|byte| *byte == marker)
    }
}

struct PlantUmlMarkdownOps;

impl PlantUmlMarkdownOps {
    fn extract(source: String) -> String {
        MermaidMarkdownOps::extract_for_languages(source, &["plantuml"])
    }
}

#[cfg(test)]
#[path = "diagram_source_tests.rs"]
mod tests;
