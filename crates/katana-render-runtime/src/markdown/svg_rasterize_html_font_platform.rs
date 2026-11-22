use resvg::usvg::fontdb::{Database, Family, ID};
#[cfg(not(target_os = "macos"))]
use resvg::usvg::fontdb::{Query, Style, Weight};
use std::{collections::BTreeSet, ops::ControlFlow, path::PathBuf};

#[path = "svg_rasterize_html_font_platform_fontconfig.rs"]
mod fontconfig;
use fontconfig::FontconfigFonts;

#[cfg(target_os = "macos")]
pub(in crate::markdown::svg_rasterize) const INTERFACE_BOLD_WEIGHT: u16 = 700;

pub(in crate::markdown::svg_rasterize) trait PlatformFontStrategy {
    fn installed_paths(&self, families: &BTreeSet<&str>) -> BTreeSet<PathBuf>;

    fn installed_system_ui_family(&self) -> Option<&'static str> {
        None
    }

    fn cjk_fallback_family(&self) -> Option<&'static str> {
        None
    }

    #[cfg(not(target_os = "macos"))]
    fn cjk_fallback_face(
        &self,
        database: &Database,
        character: char,
        weight: u16,
        style: Style,
    ) -> Option<ID> {
        if !super::super::request::is_cjk_character(character) {
            return None;
        }
        let family = self.cjk_fallback_family()?;
        database.query(&Query {
            families: &[Family::Name(family)],
            weight: Weight(weight),
            style,
            ..Query::default()
        })
    }

    fn system_alias(&self) -> Family<'static> {
        Family::SansSerif
    }

    fn html_sans_serif_family(&self) -> &'static str {
        "Arial"
    }

    fn control_arial_family(&self) -> &'static str {
        "Arial"
    }

    fn compatible_named_family(&self, _: &str) -> Option<&'static str> {
        None
    }

    fn localized_sans_serif(&self, _: &str) -> Option<&'static str> {
        None
    }

    fn face_override(
        &self,
        _: &Database,
        _: &str,
        _: &[String],
        _: u16,
        _: bool,
    ) -> ControlFlow<Option<ID>> {
        ControlFlow::Continue(())
    }
}

pub(in crate::markdown::svg_rasterize) fn font_platform() -> impl PlatformFontStrategy {
    #[cfg(any(windows, target_os = "macos", target_os = "ios"))]
    let selected = NativeFonts;
    #[cfg(not(any(windows, target_os = "macos", target_os = "ios")))]
    let selected = FontconfigFonts;
    selected
}

pub(in crate::markdown::svg_rasterize) fn localized_font_family(
    family: &str,
    localized: &str,
) -> Option<String> {
    let is_generic = |name: &str| name.trim().eq_ignore_ascii_case("sans-serif");
    family.split(',').any(is_generic).then(|| {
        family
            .split(',')
            .map(str::trim)
            .filter(|name| {
                !matches!(
                    name.to_ascii_lowercase().as_str(),
                    "-apple-system" | "blinkmacsystemfont"
                )
            })
            .map(|name| if is_generic(name) { localized } else { name })
            .collect::<Vec<_>>()
            .join(", ")
    })
}

#[cfg(any(windows, target_os = "macos", target_os = "ios"))]
struct NativeFonts;

#[cfg(any(windows, target_os = "macos", target_os = "ios"))]
impl PlatformFontStrategy for NativeFonts {
    fn installed_paths(&self, families: &BTreeSet<&str>) -> BTreeSet<PathBuf> {
        let paths = super::native_system_font_paths(families);
        if paths.is_empty() {
            FontconfigFonts.installed_paths(families)
        } else {
            paths
        }
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn system_alias(&self) -> Family<'static> {
        Family::Name("System Font")
    }

    #[cfg(target_os = "macos")]
    fn html_sans_serif_family(&self) -> &'static str {
        /* WHY: macOSのHTML sans-serifはChromeの実選択に合わせる。 */
        "Helvetica"
    }

    #[cfg(target_os = "macos")]
    fn face_override(
        &self,
        database: &Database,
        family: &str,
        names: &[String],
        weight: u16,
        italic: bool,
    ) -> ControlFlow<Option<ID>> {
        use super::super::sources::{bold_alias_id, is_internal_family};

        if is_internal_family(family) || names.iter().any(|name| is_internal_family(name)) {
            return ControlFlow::Break(None);
        }
        let family = family.trim().to_ascii_lowercase();
        if weight == INTERFACE_BOLD_WEIGHT
            && !italic
            && matches!(
                family.as_str(),
                ".hiragino kaku gothic interface" | "hiragino kaku gothic interface"
            )
            && let Some(selected) = bold_alias_id(database)
        {
            return ControlFlow::Break(Some(selected));
        }
        ControlFlow::Continue(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Database, Family, FontconfigFonts, PlatformFontStrategy, font_platform};
    use std::collections::BTreeSet;
    use std::ops::ControlFlow;

    struct DefaultPlatform;

    impl PlatformFontStrategy for DefaultPlatform {
        fn installed_paths(&self, _: &BTreeSet<&str>) -> BTreeSet<std::path::PathBuf> {
            BTreeSet::new()
        }
    }

    #[test]
    fn fontconfig_keeps_generic_system_alias_and_ordinary_face_selection() {
        let strategy = FontconfigFonts;
        assert_eq!(strategy.system_alias(), Family::SansSerif);
        assert_eq!(
            strategy.face_override(&Database::new(), "Arial", &[], 400, false),
            ControlFlow::Continue(())
        );
    }

    #[test]
    fn default_platform_keeps_optional_families_unconfigured() {
        let strategy = DefaultPlatform;
        assert!(
            strategy
                .installed_paths(&BTreeSet::from(["Arial"]))
                .is_empty()
        );
        assert_eq!(strategy.installed_system_ui_family(), None);
        assert_eq!(strategy.cjk_fallback_family(), None);
        assert_eq!(strategy.html_sans_serif_family(), "Arial");
        assert_eq!(strategy.control_arial_family(), "Arial");
        assert_eq!(strategy.compatible_named_family("Arial"), None);
        assert_eq!(strategy.localized_sans_serif("ja-JP"), None);
    }

    #[test]
    fn selected_platform_owns_html_generic_sans_serif_family() {
        let expected = if cfg!(target_os = "macos") {
            "Helvetica"
        } else {
            "Arial"
        };
        assert_eq!(font_platform().html_sans_serif_family(), expected);
    }
}
