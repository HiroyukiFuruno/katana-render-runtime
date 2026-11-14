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
        let mut fence: Option<(&'static str, u8, usize)> = None;
        for line in source.lines() {
            if fence.is_none() {
                fence = Self::starts_block(line, languages);
                continue;
            }
            let Some((language, marker, marker_count)) = fence else {
                continue;
            };
            if Self::ends_block(line, marker, marker_count) {
                let content = lines.join("\n");
                return if language == "zenuml" {
                    format!("zenuml\n{content}")
                } else {
                    content
                };
            }
            lines.push(line);
        }
        source
    }

    fn starts_block(line: &str, languages: &[&'static str]) -> Option<(&'static str, u8, usize)> {
        let trimmed = line.trim();
        Self::language_of(trimmed, b'`', languages)
            .or_else(|| Self::language_of(trimmed, b'~', languages))
    }

    fn language_of(
        line: &str,
        marker: u8,
        languages: &[&'static str],
    ) -> Option<(&'static str, u8, usize)> {
        let bytes = line.as_bytes();
        let marker_count = bytes.iter().take_while(|it| **it == marker).count();
        if marker_count < MIN_MARKDOWN_FENCE_MARKERS {
            return None;
        }
        let language = line[marker_count..].split_whitespace().next()?;
        languages
            .iter()
            .copied()
            .find(|it| *it == language)
            .map(|language| (language, marker, marker_count))
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
