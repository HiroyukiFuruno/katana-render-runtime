use super::native_text::{
    HEADING_END_MARKER, HEADING_LEVEL_1, HEADING_LEVEL_6, HEADING_SEP_MARKER, HEADING_START_MARKER,
};
use super::regex_ops::RegexOps;
use crate::markdown::MarkdownError;

pub(super) fn body_content(html: &str) -> Result<String, MarkdownError> {
    let lower = html.to_ascii_lowercase();
    let start = lower
        .find("<body")
        .and_then(|pos| lower[pos..].find('>').map(|end| pos + end + 1))
        .unwrap_or(0);
    let end = lower.rfind("</body>").unwrap_or(html.len());
    Ok(html[start..end.max(start)].to_string())
}

/* Mark <h1>…</h6> with control-char markers that survive tag stripping.
regex crate does not support backreferences, so iterate per heading level. */
pub(super) fn mark_headings(html: &str) -> Result<String, MarkdownError> {
    let mut result = html.to_string();
    for level in HEADING_LEVEL_1..=HEADING_LEVEL_6 {
        let pattern = format!(r"(?is)<h{level}\b[^>]*>(.*?)</h{level}>");
        let regex = RegexOps::compile(&pattern)?;
        result = regex
            .replace_all(&result, |caps: &regex::Captures| {
                let (_, [content]) = caps.extract();
                format!(
                    "\n{HEADING_START_MARKER}h{level}{HEADING_SEP_MARKER}{content}{HEADING_END_MARKER}\n",
                )
            })
            .to_string();
    }
    Ok(result)
}

pub(super) fn replace_image_alt(html: &str) -> Result<String, MarkdownError> {
    let regex = RegexOps::compile(r#"(?is)<img\b[^>]*\balt="([^"]*)"[^>]*>"#)?;
    Ok(regex
        .replace_all(html, |captures: &regex::Captures| {
            let (_, [image_text]) = captures.extract();
            format!("\n[image: {}]\n", decode_entities(image_text))
        })
        .to_string())
}

pub(super) fn remove_tag_blocks(html: &str, tag: &str) -> Result<String, MarkdownError> {
    let pattern = format!(r"(?is)<{tag}\b[^>]*>.*?</{tag}>");
    let regex = RegexOps::compile(&pattern)?;
    Ok(regex.replace_all(html, "\n").to_string())
}

pub(super) fn block_tags_to_breaks(html: &str) -> Result<String, MarkdownError> {
    Ok(replace_matching_tags(html, is_block_tag, "\n"))
}

pub(super) fn strip_tags(html: &str) -> String {
    replace_matching_tags(html, |_| true, " ")
}

pub(super) fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn replace_matching_tags(
    html: &str,
    predicate: impl Fn(&str) -> bool,
    replacement: &str,
) -> String {
    let mut result = String::new();
    let mut remaining = html;
    while let Some(start) = remaining.find('<') {
        let Some(end) = remaining[start..].find('>') else {
            break;
        };
        let tag = &remaining[start + 1..start + end];
        result.push_str(&remaining[..start]);
        if predicate(tag_name(tag).as_str()) {
            result.push_str(replacement);
        } else {
            result.push_str(&remaining[start..=start + end]);
        }
        remaining = &remaining[start + end + 1..];
    }
    result.push_str(remaining);
    result
}

fn tag_name(tag: &str) -> String {
    let Some(name) = tag.trim_start_matches('/').split_whitespace().next() else {
        return String::new();
    };
    name.trim_end_matches('/').to_ascii_lowercase()
}

fn is_block_tag(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "div"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "li"
            | "tr"
            | "table"
            | "pre"
            | "blockquote"
            | "br"
            | "hr"
    )
}

#[cfg(test)]
#[path = "native_text_parser_html_tests.rs"]
mod tests;
