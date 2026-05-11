#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MermaidDiagramType {
    Zenuml,
    Other,
}

impl MermaidDiagramType {
    pub(super) fn from_source(source: &str) -> Self {
        match Self::diagram_type_token(source) {
            Some(token) if token.eq_ignore_ascii_case("zenuml") => Self::Zenuml,
            _ => Self::Other,
        }
    }

    pub(super) fn request_value(self) -> &'static str {
        match self {
            Self::Zenuml => "zenuml",
            Self::Other => "",
        }
    }

    fn diagram_type_token(source: &str) -> Option<String> {
        let lines: Vec<&str> = source
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        let line = Self::diagram_type_line(&lines)?;
        line.split_whitespace().next().map(str::to_string)
    }

    fn diagram_type_line<'a>(lines: &'a [&str]) -> Option<&'a str> {
        match lines.first().copied() {
            Some("---") => Self::frontmatter_body_line(lines),
            first => first,
        }
    }

    fn frontmatter_body_line<'a>(lines: &'a [&str]) -> Option<&'a str> {
        let end_index = lines.iter().skip(1).position(|line| *line == "---")?;
        lines.get(end_index + 2).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::MermaidDiagramType;

    #[test]
    fn detects_zenuml_after_frontmatter_and_blank_lines() {
        let source = "---\ntitle: Sample\n---\n\n  zenuml\nA.method()";

        let diagram_type = MermaidDiagramType::from_source(source);

        assert_eq!(diagram_type, MermaidDiagramType::Zenuml);
        assert_eq!(diagram_type.request_value(), "zenuml");
    }

    #[test]
    fn keeps_other_mermaid_diagrams_on_existing_path() {
        let diagram_type = MermaidDiagramType::from_source("\n graph TD; A-->B");

        assert_eq!(diagram_type, MermaidDiagramType::Other);
        assert_eq!(diagram_type.request_value(), "");
    }

    #[test]
    fn detects_committed_zenuml_fixtures_after_markdown_extraction() {
        for fixture in [EN_ZENUML_FIXTURE, JA_ZENUML_FIXTURE] {
            let source = fixture_mermaid_source(fixture);

            assert_eq!(
                MermaidDiagramType::from_source(&source),
                MermaidDiagramType::Zenuml
            );
        }
    }

    #[test]
    fn fixture_source_returns_empty_when_mermaid_fence_is_unclosed() {
        let source = fixture_mermaid_source("~~~mermaid\nzenuml\nA.method()");

        assert!(source.is_empty());
    }

    fn fixture_mermaid_source(markdown: &str) -> String {
        let mut lines = Vec::new();
        let mut in_mermaid = false;
        for line in markdown.lines() {
            if line.trim_start().starts_with("~~~mermaid") {
                in_mermaid = true;
                continue;
            }
            if in_mermaid && line.trim() == "~~~" {
                return lines.join("\n");
            }
            if in_mermaid {
                lines.push(line);
            }
        }
        String::new()
    }

    const EN_ZENUML_FIXTURE: &str =
        include_str!("../../../../../tests/fixtures/mermaid/en/28-zen-uml.md");
    const JA_ZENUML_FIXTURE: &str =
        include_str!("../../../../../tests/fixtures/mermaid/ja/28-zen-uml.md");
}
