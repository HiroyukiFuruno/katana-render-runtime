const CORETEXT_TEST_POINT_SIZE: f64 = 12.0;

type NativeFont = objc2_core_foundation::CFRetained<objc2_core_text::CTFont>;

pub(super) fn japanese_ui_font_pair() -> Result<(NativeFont, NativeFont), String> {
    use objc2_core_foundation::{CFRange, CFString};
    use objc2_core_text::CTFontUIFontType;

    let language = CFString::from_str("ja");
    let regular = create_ui_font(CTFontUIFontType::System, &language)
        .ok_or("CoreText could not create the regular UI font")?;
    let bold = create_ui_font(CTFontUIFontType::EmphasizedSystem, &language)
        .ok_or("CoreText could not create the emphasized UI font")?;
    let sample = CFString::from_str("装飾面");
    let range = CFRange {
        location: 0,
        length: sample.length(),
    };
    Ok((
        for_japanese_sample(regular, &sample, range),
        for_japanese_sample(bold, &sample, range),
    ))
}

fn create_ui_font(
    kind: objc2_core_text::CTFontUIFontType,
    language: &objc2_core_foundation::CFString,
) -> Option<objc2_core_foundation::CFRetained<objc2_core_text::CTFont>> {
    /* WHY: CoreTextへ公開UI font種別、日本語language、point sizeを渡す。 */
    unsafe {
        objc2_core_text::CTFont::new_ui_font_for_language(
            kind,
            CORETEXT_TEST_POINT_SIZE,
            Some(language),
        )
    }
}

fn for_japanese_sample(
    font: objc2_core_foundation::CFRetained<objc2_core_text::CTFont>,
    sample: &objc2_core_foundation::CFString,
    range: objc2_core_foundation::CFRange,
) -> objc2_core_foundation::CFRetained<objc2_core_text::CTFont> {
    /* WHY: rangeはsampleのCFString.length()で得たUTF-16全範囲。 */
    unsafe { font.for_string(sample, range) }
}

pub(super) fn assert_coretext_interface_face(
    font: &objc2_core_text::CTFont,
    expected_postscript: &str,
    expected_file_suffix: &str,
    expected_bold: bool,
) -> Result<(), String> {
    let (postscript, path, bold) = super::super::interface_bold::coretext_face_identity(font)
        .ok_or("CoreText face identity could not be read")?;
    assert_eq!(postscript, expected_postscript);
    let filename = path.file_name().and_then(|name| name.to_str());
    assert!(filename.is_some_and(|name| name.ends_with(expected_file_suffix)));
    assert_eq!(bold, expected_bold);
    Ok(())
}
