use super::{NativeHtmlFallback, explicit_variations, full_range, named_base_font, valid_size};
use crate::markdown::svg_rasterize::font::platform::INTERFACE_BOLD_WEIGHT;
use objc2_core_foundation::{CFRetained, CFString};
use objc2_core_text::{CTFont, CTFontUIFontType};
use resvg::usvg::fontdb::{Database, ID};

const NORMAL_FONT_WEIGHT: u16 = 400;

pub(in crate::markdown::svg_rasterize) enum NativeHtmlFallbackBase<'a> {
    SystemUi,
    FileBackedSelectedFace { database: &'a Database, id: ID },
}

pub(in crate::markdown::svg_rasterize) struct NativeHtmlFallbackQuery {
    base: CFRetained<CTFont>,
    language: Option<CFRetained<CFString>>,
}

impl NativeHtmlFallbackQuery {
    pub(in crate::markdown::svg_rasterize) fn new(
        base: NativeHtmlFallbackBase<'_>,
        size: f64,
        weight: u16,
        effective_lang: Option<&str>,
    ) -> Option<Self> {
        let language = effective_lang
            .filter(|value| !value.is_empty())
            .map(CFString::from_str);
        let base = native_html_base_font(base, size, weight, language.as_deref())?;
        Some(Self { base, language })
    }

    pub(in crate::markdown::svg_rasterize) fn select(
        &self,
        character: char,
    ) -> Option<NativeHtmlFallback> {
        let font =
            fallback_for_string_with_language(&self.base, character, self.language.as_deref());
        Some(NativeHtmlFallback {
            explicit_variations: explicit_variations(&font)?,
            font,
        })
    }
}

#[cfg(test)]
pub(in crate::markdown::svg_rasterize) fn native_html_fallback(
    base: NativeHtmlFallbackBase<'_>,
    character: char,
    size: f64,
    weight: u16,
    effective_lang: Option<&str>,
) -> Option<NativeHtmlFallback> {
    NativeHtmlFallbackQuery::new(base, size, weight, effective_lang)?.select(character)
}

fn native_html_base_font(
    base: NativeHtmlFallbackBase<'_>,
    size: f64,
    weight: u16,
    language: Option<&CFString>,
) -> Option<CFRetained<CTFont>> {
    valid_size(size)?;
    match base {
        NativeHtmlFallbackBase::SystemUi => system_ui_font(size, weight, language),
        NativeHtmlFallbackBase::FileBackedSelectedFace { database, id } => {
            named_base_font(database, id, size)
        }
    }
}

fn system_ui_font(
    size: f64,
    weight: u16,
    language: Option<&CFString>,
) -> Option<CFRetained<CTFont>> {
    let kind = match weight {
        NORMAL_FONT_WEIGHT => CTFontUIFontType::System,
        INTERFACE_BOLD_WEIGHT => CTFontUIFontType::EmphasizedSystem,
        _ => return None,
    };
    /* WHY: UI種別をCoreTextへ渡し、OSごとの実際のsystem-uiを選ばせる。 */
    unsafe { CTFont::new_ui_font_for_language(kind, size, language) }
}

fn fallback_for_string_with_language(
    font: &CTFont,
    character: char,
    language: Option<&CFString>,
) -> CFRetained<CTFont> {
    let text = CFString::from_str(&character.to_string());
    let range = full_range(&text);
    /* WHY: languageはHTML側で確定したものだけをCoreText fallbackへ渡す。 */
    unsafe { font.for_string_with_language(&text, range, language) }
}
