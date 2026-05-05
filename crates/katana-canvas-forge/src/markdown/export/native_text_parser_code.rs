use super::native_text::{CODE_FENCE_MARKER, NativeTextLine, NativeTextSpan};
use super::regex_ops::RegexOps;
use crate::markdown::MarkdownError;

type CodeBlocks = Vec<Vec<NativeTextLine>>;
type CodeBlockExtraction = (String, CodeBlocks);

/* Extract <pre><code> blocks before tag stripping, replace with CODE_FENCE markers */
pub(super) fn extract_code_blocks(
    html: &str,
    is_dark: bool,
) -> Result<CodeBlockExtraction, MarkdownError> {
    let regex = code_block_regex()?;
    let code_blocks = collect_code_blocks(&regex, html, is_dark)?;
    let result = replace_code_blocks(&regex, html);
    Ok((result, code_blocks))
}

fn code_block_regex() -> Result<regex::Regex, MarkdownError> {
    RegexOps::compile(r#"(?is)<pre\b[^>]*><code\b([^>]*)>(.*?)</code></pre>"#)
}

fn collect_code_blocks(
    regex: &regex::Regex,
    html: &str,
    is_dark: bool,
) -> Result<CodeBlocks, MarkdownError> {
    let mut code_blocks = Vec::new();
    for caps in regex.captures_iter(html) {
        let (_, [attrs, raw_code]) = caps.extract();
        push_code_block(&mut code_blocks, attrs, raw_code, is_dark)?;
    }
    Ok(code_blocks)
}

fn push_code_block(
    code_blocks: &mut CodeBlocks,
    attrs: &str,
    raw_code: &str,
    is_dark: bool,
) -> Result<(), MarkdownError> {
    let code = super::native_text_parser_html::decode_entities(raw_code);
    let language = extract_code_language(attrs);
    code_blocks.push(highlight_code(&code, language.as_deref(), is_dark)?);
    Ok(())
}

fn replace_code_blocks(regex: &regex::Regex, html: &str) -> String {
    let mut index = 0;
    regex
        .replace_all(html, |_captures: &regex::Captures| {
            let current = index;
            index += 1;
            format!("\n{CODE_FENCE_MARKER}{current}{CODE_FENCE_MARKER}\n")
        })
        .to_string()
}

fn extract_code_language(attrs: &str) -> Option<String> {
    let regex = RegexOps::compile(r#"(?i)class="language-([^"\s]+)""#).ok()?;
    regex
        .captures(attrs)
        .and_then(|c| c.get(1).map(|capture| capture.as_str().to_lowercase()))
}

fn highlight_code(
    code: &str,
    language: Option<&str>,
    is_dark: bool,
) -> Result<Vec<NativeTextLine>, MarkdownError> {
    use syntect::easy::HighlightLines;
    use syntect::util::LinesWithEndings;

    let theme = &theme_set().themes[theme_name(is_dark)];
    let syntax = highlight_syntax(language);
    let mut highlighter = HighlightLines::new(syntax, theme);
    LinesWithEndings::from(code)
        .map(|line| highlight_line(&mut highlighter, line))
        .collect()
}

fn theme_name(is_dark: bool) -> &'static str {
    if is_dark {
        return "base16-ocean.dark";
    }
    "InspiredGitHub"
}

fn syntax_set() -> &'static syntect::parsing::SyntaxSet {
    static SYNTAX_SET: std::sync::LazyLock<syntect::parsing::SyntaxSet> =
        std::sync::LazyLock::new(syntect::parsing::SyntaxSet::load_defaults_newlines);
    &SYNTAX_SET
}

fn theme_set() -> &'static syntect::highlighting::ThemeSet {
    static THEME_SET: std::sync::LazyLock<syntect::highlighting::ThemeSet> =
        std::sync::LazyLock::new(syntect::highlighting::ThemeSet::load_defaults);
    &THEME_SET
}

fn highlight_syntax(language: Option<&str>) -> &'static syntect::parsing::SyntaxReference {
    language
        .and_then(|lang| syntax_set().find_syntax_by_token(lang))
        .unwrap_or_else(|| syntax_set().find_syntax_plain_text())
}

fn highlight_line(
    highlighter: &mut syntect::easy::HighlightLines,
    line_str: &str,
) -> Result<NativeTextLine, MarkdownError> {
    let text = line_str.trim_end_matches(['\n', '\r']).to_string();
    let ranges = highlight_result(highlighter.highlight_line(line_str, syntax_set()))?;
    Ok(NativeTextLine::code_highlighted(
        text,
        highlighted_spans(&ranges),
    ))
}

fn highlighted_spans(ranges: &[(syntect::highlighting::Style, &str)]) -> Vec<NativeTextSpan> {
    ranges
        .iter()
        .filter(|(_, text)| !text.is_empty() && *text != "\n" && *text != "\r\n")
        .map(|(style, text)| NativeTextSpan {
            text: text.trim_end_matches(['\n', '\r']).to_string(),
            color: [style.foreground.r, style.foreground.g, style.foreground.b],
        })
        .filter(|span| !span.text.is_empty())
        .collect()
}

fn highlight_result<T>(result: Result<T, syntect::Error>) -> Result<T, MarkdownError> {
    result.map_err(|error| MarkdownError::ExportFailed(error.to_string()))
}

#[cfg(test)]
#[path = "native_text_parser_code_tests.rs"]
mod tests;
