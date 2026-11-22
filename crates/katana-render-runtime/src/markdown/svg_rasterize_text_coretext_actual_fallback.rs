use super::font_identity;
use crate::markdown::svg_rasterize::font::platform::INTERFACE_BOLD_WEIGHT;
use objc2_core_foundation::{CFDictionary, CFNumber, CFRange, CFRetained, CFString};
use objc2_core_text::{CTFont, CTFontUIFontType};
use resvg::usvg::fontdb::{Database, ID};
use std::collections::BTreeMap;

pub(in crate::markdown::svg_rasterize) use font_identity::ActualCoreTextFace;

const NORMAL_FONT_WEIGHT: u16 = 400;
const MAX_EXPLICIT_VARIATIONS: usize = 16;
const OPENTYPE_TAG_BYTES: usize = 4;

pub(in crate::markdown::svg_rasterize) enum NativeHtmlFallbackBase<'a> {
    SystemUi,
    FileBackedSelectedFace { database: &'a Database, id: ID },
}

pub(in crate::markdown::svg_rasterize) struct NativeHtmlFallback {
    pub(in crate::markdown::svg_rasterize) font: CFRetained<CTFont>,
    pub(in crate::markdown::svg_rasterize) explicit_variations:
        BTreeMap<[u8; OPENTYPE_TAG_BYTES], f64>,
}

pub(in crate::markdown::svg_rasterize) fn actual_face_for(
    font: &CTFont,
) -> Option<ActualCoreTextFace> {
    font_identity::resolve_actual_face(font)
}

pub(in crate::markdown::svg_rasterize) fn actual_font_metadata_for(
    font: &CTFont,
) -> Option<font_identity::ActualCoreTextFontMetadata> {
    font_identity::actual_font_metadata(font)
}

pub(super) fn default_fallback_font(
    database: &Database,
    base_id: ID,
    character: char,
    size: f64,
) -> Option<CFRetained<CTFont>> {
    let base = named_base_font(database, base_id, size)?;
    Some(fallback_for_string(&base, character))
}

pub(in crate::markdown::svg_rasterize) fn native_html_fallback(
    base: NativeHtmlFallbackBase<'_>,
    character: char,
    size: f64,
    weight: u16,
    effective_lang: Option<&str>,
) -> Option<NativeHtmlFallback> {
    let language = effective_lang
        .filter(|value| !value.is_empty())
        .map(CFString::from_str);
    let base = native_html_base_font(base, size, weight, language.as_deref())?;
    let font = fallback_for_string_with_language(&base, character, language.as_deref());
    Some(NativeHtmlFallback {
        explicit_variations: explicit_variations(&font)?,
        font,
    })
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

fn valid_size(size: f64) -> Option<()> {
    (size.is_finite() && size > 0.0).then_some(())
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

fn named_base_font(database: &Database, id: ID, size: f64) -> Option<CFRetained<CTFont>> {
    valid_size(size)?;
    let postscript_name = &database.face(id)?.post_script_name;
    (!postscript_name.is_empty()).then_some(())?;
    let name = CFString::from_str(postscript_name);
    /* WHY: CoreTextが別名へ置換したbaseをcascadeの起点にしない。 */
    let font = unsafe { CTFont::with_name(&name, size, std::ptr::null()) };
    (unsafe { font.post_script_name() }.to_string() == postscript_name.as_str()).then_some(())?;
    font_identity::matches(database, id, &font).then_some(font)
}

fn fallback_for_string(font: &CTFont, character: char) -> CFRetained<CTFont> {
    let text = CFString::from_str(&character.to_string());
    let range = full_range(&text);
    /* WHY: CFStringのUTF-16長を使い、補助平面文字も範囲に収める。 */
    unsafe { font.for_string(&text, range) }
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

fn full_range(text: &CFString) -> CFRange {
    CFRange {
        location: 0,
        length: text.length(),
    }
}

fn explicit_variations(font: &CTFont) -> Option<BTreeMap<[u8; OPENTYPE_TAG_BYTES], f64>> {
    let Some(variations) = (unsafe { font.variation() }) else {
        return Some(BTreeMap::new());
    };
    /* WHY: CoreText契約の数値dictionaryだけを、有限個のOpenType軸へ絞る。 */
    let variations = unsafe { variations.cast_unchecked::<CFNumber, CFNumber>() };
    (variations.len() <= MAX_EXPLICIT_VARIATIONS).then_some(())?;
    variation_pairs(&variations)
}

fn variation_pairs(
    variations: &CFDictionary<CFNumber, CFNumber>,
) -> Option<BTreeMap<[u8; OPENTYPE_TAG_BYTES], f64>> {
    let (tags, values) = variations.to_vecs();
    tags.into_iter()
        .zip(values)
        .try_fold(BTreeMap::new(), |mut coordinates, (tag, value)| {
            let tag = variation_tag(&tag)?;
            let value = value.as_f64().filter(|value| value.is_finite())?;
            coordinates
                .insert(tag, value)
                .is_none()
                .then_some(coordinates)
        })
}

fn variation_tag(identifier: &CFNumber) -> Option<[u8; OPENTYPE_TAG_BYTES]> {
    let tag = u32::try_from(identifier.as_i64()?).ok()?.to_be_bytes();
    tag.iter()
        .all(|byte| byte.is_ascii_graphic() || *byte == b' ')
        .then_some(tag)
}

#[cfg(test)]
mod tests {
    use super::{
        Database, ID, INTERFACE_BOLD_WEIGHT, NativeHtmlFallback, NativeHtmlFallbackBase,
        actual_face_for, native_html_fallback,
    };

    #[test]
    fn native_fallback_requires_a_verified_named_base() {
        let database = Database::new();
        let base = NativeHtmlFallbackBase::FileBackedSelectedFace {
            database: &database,
            id: ID::dummy(),
        };
        assert!(native_html_fallback(base, '装', 16.0, 400, Some("zh-Hans")).is_none());
    }

    #[test]
    fn native_ui_fallback_preserves_the_selected_language_and_explicit_weight() -> Result<(), String>
    {
        for (locale, character, expected_name, expected_weight) in [
            ("ja", '装', ".HiraKakuInterface-W4", None),
            ("zh-Hans", '装', ".PingFangUITextSC-Regular", Some(400.0)),
            ("zh-Hans", 'を', ".CJKSymbolsFallbackSC-Regular", None),
        ] {
            let selection = native_html_fallback(
                NativeHtmlFallbackBase::SystemUi,
                character,
                16.0,
                400,
                Some(locale),
            )
            .ok_or_else(|| format!("CoreText fallback missing for {locale}"))?;
            assert_actual_selection(&selection, expected_name, expected_weight)?;
        }
        Ok(())
    }

    #[test]
    fn native_ui_bold_display_resolves_the_physical_collection_face() -> Result<(), String> {
        const DISPLAY_SIZE: f64 = 32.0;
        let selection = native_html_fallback(
            NativeHtmlFallbackBase::SystemUi,
            '装',
            DISPLAY_SIZE,
            INTERFACE_BOLD_WEIGHT,
            Some("zh-Hans"),
        )
        .ok_or("CoreText bold display fallback missing")?;
        assert_actual_selection(
            &selection,
            ".PingFangUIDisplaySC-Bold",
            Some(f64::from(INTERFACE_BOLD_WEIGHT)),
        )
    }

    fn assert_actual_selection(
        selection: &NativeHtmlFallback,
        expected_name: &str,
        expected_weight: Option<f64>,
    ) -> Result<(), String> {
        /* WHY: 同じCTFontから得たphysical faceとvariationを一組で検証する。 */
        let actual_face = actual_face_for(&selection.font).ok_or("physical face missing")?;
        assert_eq!(actual_face.post_script_name, expected_name);
        assert_eq!(
            unsafe { selection.font.post_script_name() }.to_string(),
            expected_name
        );
        assert_eq!(
            selection.explicit_variations.get(b"wght").copied(),
            expected_weight
        );
        let bytes =
            std::fs::read(&actual_face.canonical_path).map_err(|error| error.to_string())?;
        rustybuzz::ttf_parser::Face::parse(&bytes, actual_face.face_index)
            .map_err(|error| format!("{error:?}"))?;
        Ok(())
    }
}
