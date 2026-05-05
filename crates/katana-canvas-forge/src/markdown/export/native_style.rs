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
            background_color: css_value(&body_css, "background-color")
                .unwrap_or_else(|| DEFAULT_BACKGROUND_COLOR.to_string()),
            text_color: css_value(&body_css, "color")
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
    let regex = regex::Regex::new(r"(?is)body\s*\{([^}]*)\}").ok()?;
    regex
        .captures(html)
        .and_then(|captures| captures.get(1))
        .map(|it| it.as_str().to_string())
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
    use super::{NativeDocumentStyle, body_css, css_value};

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
}
