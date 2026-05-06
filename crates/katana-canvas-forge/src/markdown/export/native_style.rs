const DEFAULT_BACKGROUND_COLOR: &str = "#ffffff";
const DEFAULT_TEXT_COLOR: &str = "#222222";

pub(crate) struct NativeDocumentStyle {
    background_color: String,
    text_color: String,
}

impl NativeDocumentStyle {
    pub(crate) fn parse(html: &str) -> Self {
        let Some(body_css) = body_css(html) else {
            return Self::default();
        };
        Self {
            background_color: css_color_value(&body_css, "background-color")
                .or_else(|| css_color_value(&body_css, "background"))
                .unwrap_or_else(|| DEFAULT_BACKGROUND_COLOR.to_string()),
            text_color: css_color_value(&body_css, "color")
                .unwrap_or_else(|| DEFAULT_TEXT_COLOR.to_string()),
        }
    }

    pub(crate) fn background_color(&self) -> &str {
        &self.background_color
    }

    pub(crate) fn text_color(&self) -> &str {
        &self.text_color
    }
}

impl Default for NativeDocumentStyle {
    fn default() -> Self {
        Self {
            background_color: DEFAULT_BACKGROUND_COLOR.to_string(),
            text_color: DEFAULT_TEXT_COLOR.to_string(),
        }
    }
}

fn body_css(html: &str) -> Option<String> {
    let style_regex = regex::Regex::new(r"(?is)<style[^>]*>(.*?)</style>").ok()?;
    style_regex
        .captures_iter(html)
        .filter_map(|captures| captures.get(1))
        .find_map(|it| body_rule_css(it.as_str()))
        .or_else(|| body_rule_css(html))
}

fn body_rule_css(css: &str) -> Option<String> {
    let regex = regex::Regex::new(r"(?is)([^{}]+)\{([^}]*)\}").ok()?;
    regex
        .captures_iter(css)
        .find(|captures| {
            captures
                .get(1)
                .is_some_and(|it| selector_targets_body(it.as_str()))
        })
        .and_then(|captures| captures.get(2))
        .map(|it| it.as_str().to_string())
}

fn selector_targets_body(selectors: &str) -> bool {
    selectors.split(',').any(selector_is_body)
}

fn selector_is_body(selector: &str) -> bool {
    let selector = selector.trim().to_ascii_lowercase();
    selector
        .strip_prefix("body")
        .is_some_and(|rest| rest.is_empty() || starts_with_body_selector_suffix(rest))
}

fn starts_with_body_selector_suffix(rest: &str) -> bool {
    rest.as_bytes()
        .first()
        .is_some_and(|it| matches!(it, b'.' | b'#' | b':' | b'[' | b' ' | b'\t' | b'\n' | b'\r'))
}

fn css_color_value(css: &str, name: &str) -> Option<String> {
    css_value(css, name).and_then(|it| first_css_color(&it))
}

fn first_css_color(value: &str) -> Option<String> {
    value
        .split_whitespace()
        .map(|it| it.trim_matches(','))
        .find(|it| it.starts_with('#') || it.to_ascii_lowercase().starts_with("rgb"))
        .map(str::to_string)
}

fn css_value(css: &str, name: &str) -> Option<String> {
    css.split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .find_map(|(property, value)| {
            property
                .trim()
                .eq_ignore_ascii_case(name)
                .then(|| value.trim().to_string())
        })
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{NativeDocumentStyle, body_css, css_color_value, css_value};

    #[test]
    fn parses_body_colors_case_insensitively() {
        let html = r#"
            <style>
            BODY {
                color: #eeeeee;
                background-color: #111111;
            }
            </style>
        "#;

        let style = NativeDocumentStyle::parse(html);

        assert_eq!(style.background_color(), "#111111");
        assert_eq!(style.text_color(), "#eeeeee");
    }

    #[test]
    fn defaults_when_body_style_is_missing() {
        let style = NativeDocumentStyle::parse("<html><body>plain</body></html>");

        assert_eq!(style.background_color(), "#ffffff");
        assert_eq!(style.text_color(), "#222222");
    }

    #[test]
    fn css_value_ignores_empty_values() {
        assert_eq!(css_value("color: ;", "color"), None);
        assert_eq!(
            css_value("color: #222222;", "color"),
            Some("#222222".to_string())
        );
        assert!(body_css("<style>main { color: red; }</style>").is_none());

        let missing_background =
            NativeDocumentStyle::parse("<style>body { color: #333333; }</style>");
        let missing_color =
            NativeDocumentStyle::parse("<style>body { background-color: #fafafa; }</style>");

        assert_eq!(missing_background.background_color(), "#ffffff");
        assert_eq!(missing_color.text_color(), "#222222");
    }

    #[test]
    fn parses_body_selector_lists_and_background_shorthand() {
        let style = NativeDocumentStyle::parse(
            "<style>html, body { background: #1e1e1e; color: #eeeeee; }</style>",
        );

        assert_eq!(style.background_color(), "#1e1e1e");
        assert_eq!(style.text_color(), "#eeeeee");
        assert_eq!(
            css_color_value("background: #111111 url(a.png);", "background"),
            Some("#111111".to_string())
        );
        assert!(body_css("<style>tbody { color: red; }</style>").is_none());
    }
}
