use super::CssStyle;
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub(in crate::renderer::backends::html_interactive) struct FontContext {
    sans_serif: Option<&'static str>,
    localized_family: Option<Arc<str>>,
}

impl CssStyle {
    pub(in crate::renderer::backends::html_interactive) fn used_font_family(&self) -> &str {
        self.font_context
            .localized_family
            .as_deref()
            .unwrap_or(&self.font_family)
    }

    pub(super) fn resolve_font_context(&mut self, language: Option<&str>, inherited: &Self) {
        if let Some(language) = language {
            self.font_context.sans_serif =
                SvgRasterizeOps::localized_html_sans_serif(language.trim());
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::CssStyle;

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
