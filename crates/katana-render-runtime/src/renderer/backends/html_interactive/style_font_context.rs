use super::CssStyle;
use crate::markdown::svg_rasterize::{HtmlFontStyle, SvgRasterizeOps};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub(in crate::renderer::backends::html_interactive) struct FontContext {
    sans_serif: Option<&'static str>,
    localized_family: Option<Arc<str>>,
    language: Option<Arc<str>>,
}

impl CssStyle {
    pub(in crate::renderer::backends::html_interactive) fn used_font_family(&self) -> &str {
        self.font_context
            .localized_family
            .as_deref()
            .unwrap_or(&self.font_family)
    }

    pub(in crate::renderer::backends::html_interactive) fn effective_font_language(
        &self,
    ) -> Option<&str> {
        self.font_context.language.as_deref()
    }

    pub(in crate::renderer::backends::html_interactive) fn raster_font_style(
        &self,
    ) -> HtmlFontStyle<'_> {
        HtmlFontStyle {
            family: self.used_font_family(),
            size: self.font_size,
            weight: self.font_weight,
            italic: self.italic,
            language: self.effective_font_language(),
        }
    }

    pub(super) fn resolve_font_context(&mut self, language: Option<&str>, inherited: &Self) {
        if let Some(language) = language.map(str::trim) {
            self.font_context.language = (!language.is_empty()).then(|| Arc::from(language));
            self.font_context.sans_serif = SvgRasterizeOps::localized_html_sans_serif(language);
        }
        let previous = inherited.font_context.sans_serif;
        let current = self.font_context.sans_serif;
        if current != previous || (current.is_some() && self.font_family != inherited.font_family) {
            self.localize_font_family();
        }
    }

    pub(in crate::renderer::backends::html_interactive) fn set_font_family(
        &mut self,
        family: String,
    ) {
        self.font_family = family;
        self.localize_font_family();
    }

    fn localize_font_family(&mut self) {
        self.font_context.localized_family = self
            .font_context
            .sans_serif
            .and_then(|sans_serif| {
                SvgRasterizeOps::localized_html_font_family(&self.font_family, sans_serif)
            })
            .map(Arc::from);
    }
}

#[cfg(test)]
mod tests {
    use super::CssStyle;

    #[test]
    fn font_family_recomputes_the_inherited_localized_fallback() {
        let mut inherited = CssStyle::browser_default();
        inherited.font_context.sans_serif = Some("Test Sans, sans-serif");
        inherited.set_font_family(" -apple-system, Georgia, Sans-Serif ".into());
        assert_eq!(
            inherited.used_font_family(),
            "Georgia, Test Sans, sans-serif"
        );

        let mut authored = inherited.clone();
        authored.font_family = "blinkmacsystemfont, monospace, sans-serif".into();
        authored.resolve_font_context(None, &inherited);
        assert_eq!(
            authored.used_font_family(),
            "monospace, Test Sans, sans-serif"
        );

        authored.set_font_family("Arial".into());
        assert_eq!(authored.used_font_family(), "Arial");
    }

    #[test]
    fn effective_language_inherits_trims_and_clears_on_an_empty_override() {
        let root = CssStyle::from_element(
            "html",
            &[("lang".into(), " ja-JP ".into())],
            &CssStyle::browser_default(),
        );
        let inherited = CssStyle::from_element("p", &[], &root);
        let overridden =
            CssStyle::from_element("span", &[("lang".into(), "zh-Hans".into())], &inherited);
        let empty = CssStyle::from_element("em", &[("lang".into(), "  ".into())], &overridden);

        assert_eq!(root.effective_font_language(), Some("ja-JP"));
        assert_eq!(inherited.effective_font_language(), Some("ja-JP"));
        assert_eq!(overridden.effective_font_language(), Some("zh-Hans"));
        assert_eq!(empty.effective_font_language(), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_font_locale_localizes_inherited_generics_and_preserves_explicit_families() {
        let root = CssStyle::from_element(
            "html",
            &[
                ("lang".into(), "ja-JP".into()),
                (
                    "style".into(),
                    "font-family: -apple-system, Georgia, sans-serif".into(),
                ),
            ],
            &CssStyle::browser_default(),
        );
        let paragraph = CssStyle::from_element("p", &[], &root);
        assert_eq!(
            paragraph.used_font_family(),
            "Georgia, Noto Sans JP, Noto Sans CJK JP, sans-serif"
        );
        let inherited = CssStyle::from_element("span", &[("lang".into(), "en".into())], &paragraph);
        assert_eq!(
            inherited.used_font_family(),
            "-apple-system, Georgia, sans-serif"
        );
        let authored = CssStyle::from_element(
            "span",
            &[("style".into(), "font-family: Arial".into())],
            &root,
        );
        assert_eq!(authored.used_font_family(), "Arial");
    }
}
