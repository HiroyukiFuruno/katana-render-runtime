use crate::renderer::backends::html_interactive::style::value::{
    css_font_size, css_font_weight, css_resolved_px, split_top_level_whitespace,
};
use crate::renderer::backends::html_interactive::style::{
    normalized_property_name, CssStyle, CssTextAlign, CssTextTransform, CssWhiteSpace,
};
use crate::renderer::backends::html_interactive::style::style_typography_declaration::typography_parser::FontShorthandParser;

impl CssStyle {
    pub(crate) fn apply_typography_property(&mut self, name: &str, value: &str) -> bool {
        match normalized_property_name(name).as_ref() {
            "font" => self.apply_font_shorthand(value),
            "font-size" => self.apply_font_size(value),
            "font-weight" => self.apply_font_weight(value),
            "font-family" => self.apply_font_family(value),
            "font-feature-settings" => self.apply_font_feature_settings(value),
            "font-style" => self.apply_font_style(value),
            "text-decoration" => {
                self.underline = value.contains("underline");
                self.explicit_text_decoration = true;
            }
            "text-align" => self.apply_text_align(value),
            "text-transform" => self.apply_text_transform(value),
            "white-space" => self.apply_white_space(value),
            "letter-spacing" => self.apply_letter_spacing(value),
            _ => return false,
        }
        true
    }

    fn apply_font_size(&mut self, value: &str) {
        self.explicit_typography = true;
        self.font_size = css_font_size(
            value,
            self.font_size,
            self.viewport_width,
            self.viewport_height,
        )
        .unwrap_or(self.font_size);
    }

    fn apply_font_shorthand(&mut self, value: &str) {
        let tokens = split_top_level_whitespace(value);
        let Some((size_index, size_token, line_height, family_start)) =
            FontShorthandParser::size(&tokens, self)
        else {
            return;
        };
        let family = tokens[family_start..].join(" ");
        if family.trim().is_empty() {
            return;
        }
        let Some((italic, font_weight)) =
            FontShorthandParser::style(&tokens[..size_index], self.font_weight)
        else {
            return;
        };
        let mut parsed = self.clone();
        parsed.italic = italic;
        parsed.font_weight = font_weight;
        parsed.apply_font_size(size_token);
        parsed.apply_line_height(line_height.unwrap_or("normal"));
        parsed.apply_font_family(&family);
        *self = parsed;
    }

    fn apply_font_weight(&mut self, value: &str) {
        self.font_weight = css_font_weight(value, self.font_weight).unwrap_or(self.font_weight);
    }

    fn apply_font_style(&mut self, value: &str) {
        self.italic = matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "italic" | "oblique"
        );
    }

    fn apply_font_family(&mut self, value: &str) {
        let family = value.trim();
        if !family.is_empty() {
            self.explicit_typography = true;
            self.font_family = family.to_string();
        }
    }

    fn apply_font_feature_settings(&mut self, value: &str) {
        let settings = value.trim();
        self.font_feature_settings =
            if settings.is_empty() || settings.eq_ignore_ascii_case("normal") {
                None
            } else {
                Some(settings.to_string())
            };
    }

    fn apply_text_align(&mut self, value: &str) {
        self.text_align = match value.trim().to_ascii_lowercase().as_str() {
            "center" => CssTextAlign::Center,
            "right" | "end" => CssTextAlign::End,
            "left" | "start" => CssTextAlign::Start,
            _ => self.text_align,
        };
    }

    fn apply_text_transform(&mut self, value: &str) {
        self.text_transform = match value.trim().to_ascii_lowercase().as_str() {
            "none" => CssTextTransform::None,
            "uppercase" => CssTextTransform::Uppercase,
            "lowercase" => CssTextTransform::Lowercase,
            "capitalize" => CssTextTransform::Capitalize,
            _ => self.text_transform,
        };
    }

    fn apply_white_space(&mut self, value: &str) {
        self.white_space = match value.trim().to_ascii_lowercase().as_str() {
            "normal" => CssWhiteSpace::Normal,
            "nowrap" => CssWhiteSpace::NoWrap,
            _ => self.white_space,
        };
    }

    fn apply_letter_spacing(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("normal") {
            self.letter_spacing = 0.0;
        } else {
            self.letter_spacing = css_resolved_px(
                value,
                self.font_size,
                self.viewport_width,
                self.viewport_height,
                true,
            )
            .unwrap_or(self.letter_spacing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FontShorthandParser;
    use crate::renderer::backends::html_interactive::style::CssStyle;

    #[test]
    fn font_shorthand_with_missing_family_keeps_style_unchanged() {
        let mut style = CssStyle::browser_default();
        let original = (
            style.font_size,
            style.line_height,
            style.font_family.clone(),
            style.font_weight,
            style.italic,
        );

        style.apply_typography_property("font", "italic 16px");

        assert_eq!(
            (
                style.font_size,
                style.line_height,
                style.font_family,
                style.font_weight,
                style.italic,
            ),
            original
        );
    }

    #[test]
    fn font_shorthand_with_invalid_style_token_preserves_style() {
        let mut style = CssStyle::browser_default();
        let original = (
            style.font_size,
            style.line_height,
            style.font_family.clone(),
        );

        style.apply_typography_property("font", "invalid 16px serif");

        assert_eq!(
            (style.font_size, style.line_height, style.font_family,),
            original
        );
        assert!(FontShorthandParser::style(&["invalid"], style.font_weight).is_none());
    }
}
