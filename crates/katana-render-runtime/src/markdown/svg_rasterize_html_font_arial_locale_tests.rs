use super::{
    FINAL_GLYPH_ORIGIN, NORMAL_WEIGHT, VIEWPORT_HEIGHT, VIEWPORT_WIDTH, assert_html_frame,
    assert_valid_paint_evidence, final_span_uses_face_matching, open_html_with_audit,
};
use objc2_core_foundation::{CFRange, CFString};
use objc2_core_text::CTFont;

const ARIAL_FONT_SIZE: u32 = 26;
const ARIAL_POSTSCRIPT: &str = "ArialMT";
const HAN_FALLBACK_TEXT: &str = "骨";

#[test]
fn actual_layout_named_arial_respects_han_fallback_language() -> Result<(), String> {
    for lang in ["ja", "ko", "zh-Hans", "zh-Hant"] {
        let postscript = arial_han_coretext_fallback(lang);
        let html = format!(
            "<!doctype html><html lang='{lang}'><p style='font-family:Arial;font-size:{ARIAL_FONT_SIZE}px'>{HAN_FALLBACK_TEXT}</p>"
        );
        let (session, audit) =
            open_html_with_audit(&html, FINAL_GLYPH_ORIGIN, VIEWPORT_WIDTH, VIEWPORT_HEIGHT)?;
        assert_html_frame(&session)?;
        assert_valid_paint_evidence(&audit)?;
        let trees = audit["finalGlyphTrees"]
            .as_array()
            .ok_or_else(|| "final glyph tree evidence is absent".to_owned())?;
        assert!(
            final_span_uses_face_matching(
                trees,
                NORMAL_WEIGHT,
                |name| name == postscript,
                &[],
                is_han_fallback_span,
            ),
            "CoreText selected {postscript} for {lang} and {HAN_FALLBACK_TEXT}",
        );
    }
    Ok(())
}

fn is_han_fallback_span(span: &serde_json::Value) -> bool {
    span["requestAssociation"] == "unique_text_match"
        && span["requested"]["text"] == HAN_FALLBACK_TEXT
}

fn arial_han_coretext_fallback(language: &str) -> String {
    let name = CFString::from_str(ARIAL_POSTSCRIPT);
    let text = CFString::from_str(HAN_FALLBACK_TEXT);
    let language = CFString::from_str(language);
    let range = CFRange {
        location: 0,
        length: text.length(),
    };
    /* SAFETY: 固定PostScript名・サイズ、変形なし、文字列全体のUTF-16範囲を渡す。 */
    let face = unsafe { CTFont::with_name(&name, f64::from(ARIAL_FONT_SIZE), std::ptr::null()) };
    assert_eq!(
        unsafe { face.post_script_name() }.to_string(),
        ARIAL_POSTSCRIPT
    );
    let fallback = unsafe { face.for_string_with_language(&text, range, Some(&language)) };
    unsafe { fallback.post_script_name() }.to_string()
}
