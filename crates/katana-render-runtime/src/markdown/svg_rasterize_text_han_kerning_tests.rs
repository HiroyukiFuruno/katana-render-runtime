use super::class::{HanClass, quote_class, should_kern, should_kern_last};
use super::{CHWS, HALT, HanKerningTrim, PALT, han_kerning_features, has_layout_feature};
use skrifa::MetadataProvider;

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const HAN_FONT_BYTES: &[u8] =
    include_bytes!("../../../../tests/fixtures/fonts/han-kerning/NotoSansCJKjp-han14.otf");
const CHWS_FONT_BYTES: &[u8] = include_bytes!(
    "../../../../tests/fixtures/fonts/han-kerning/NotoSansCJKjp-Regular-subset-chws.otf"
);
const CHWS_TEXT: &str = "国）（国";

const TAG_LENGTH: usize = 4;

fn feature(tag: &[u8; TAG_LENGTH]) -> harfrust::Feature {
    harfrust::Feature::new(harfrust::Tag::from_be_bytes(*tag), 1, ..)
}

#[test]
fn malformed_font_is_rejected() {
    assert!(han_kerning_features(&[], 0, "（A）", HanKerningTrim::Normal, &[]).is_err());
}

#[test]
fn space_all_and_explicit_exclusive_features_disable_defaults() -> Result<(), String> {
    let empty = han_kerning_features(FONT_BYTES, 0, "（A）", HanKerningTrim::SpaceAll, &[])?;
    let explicit = han_kerning_features(
        FONT_BYTES,
        0,
        "（A）",
        HanKerningTrim::Normal,
        &[feature(b"palt")],
    )?;
    assert!(empty.is_empty());
    assert!(explicit.is_empty());
    Ok(())
}

#[test]
fn unsupported_font_does_not_receive_synthetic_han_features() -> Result<(), String> {
    for text in ["plain ASCII", "（あ）"] {
        let features = han_kerning_features(FONT_BYTES, 0, text, HanKerningTrim::Normal, &[])?;
        assert!(features.is_empty());
    }
    Ok(())
}

fn assert_han_font_features() -> Result<(), String> {
    let font = skrifa::FontRef::from_index(HAN_FONT_BYTES, 0).map_err(|error| error.to_string())?;
    assert!(has_layout_feature(&font, HALT));
    assert!(has_layout_feature(&font, PALT));
    assert!(!has_layout_feature(&font, CHWS));
    Ok(())
}

#[test]
fn actual_halt_font_trims_fullwidth_pairs_at_utf8_byte_boundaries() -> Result<(), String> {
    const BRACKET_BYTES: u32 = '（'.len_utf8() as u32;
    const PREFIX_BYTES: u32 = 'A'.len_utf8() as u32;
    assert_han_font_features()?;
    for (text, start, end_exclusive) in [
        ("（（", BRACKET_BYTES, BRACKET_BYTES + BRACKET_BYTES),
        ("））", 0, BRACKET_BYTES),
        (
            "A（（",
            PREFIX_BYTES + BRACKET_BYTES,
            PREFIX_BYTES + BRACKET_BYTES + BRACKET_BYTES,
        ),
        ("A））", PREFIX_BYTES, PREFIX_BYTES + BRACKET_BYTES),
    ] {
        let features = han_kerning_features(HAN_FONT_BYTES, 0, text, HanKerningTrim::Normal, &[])?;
        assert_eq!(features.len(), 1, "{text}");
        let found = &features[0];
        assert_eq!(
            (found.tag, found.value, found.start, found.end),
            (HALT, 1, start, end_exclusive - 1),
            "{text}"
        );
    }
    Ok(())
}

#[test]
fn blink_neighbor_rules_preserve_narrow_open_and_utf8_ranges() {
    const RANGE_START: usize = 3;
    const RANGE_END: usize = 6;
    assert!(should_kern(HanClass::Open, HanClass::OpenNarrow));
    assert!(should_kern_last(HanClass::CloseNarrow, HanClass::Close));
    let feature = harfrust::Feature::new(
        harfrust::Tag::from_be_bytes(*b"halt"),
        1,
        RANGE_START..RANGE_END,
    );
    assert_eq!(
        (feature.start, feature.end),
        (RANGE_START as u32, (RANGE_END - 1) as u32)
    );
}

#[test]
fn explicit_chws_zero_still_suppresses_the_default() -> Result<(), String> {
    let explicit = harfrust::Feature::new(harfrust::Tag::from_be_bytes(*b"chws"), 0, ..);
    assert!(
        han_kerning_features(FONT_BYTES, 0, "（あ）", HanKerningTrim::Normal, &[explicit])?
            .is_empty()
    );
    Ok(())
}

#[test]
fn actual_chws_font_enables_full_range_feature() -> Result<(), String> {
    let font =
        skrifa::FontRef::from_index(CHWS_FONT_BYTES, 0).map_err(|error| error.to_string())?;
    assert!(has_layout_feature(&font, CHWS));
    for character in CHWS_TEXT.chars() {
        assert!(font.charmap().map(character).is_some(), "{character}");
    }
    let features =
        han_kerning_features(CHWS_FONT_BYTES, 0, CHWS_TEXT, HanKerningTrim::Normal, &[])?;
    assert_full_range_chws(&features);
    Ok(())
}

fn assert_full_range_chws(features: &[harfrust::Feature]) {
    assert_eq!(features.len(), 1);
    let actual = &features[0];
    assert_eq!(
        (actual.tag, actual.value, actual.start, actual.end),
        (CHWS, 1, 0, u32::MAX)
    );
}

#[test]
fn actual_chws_font_respects_explicit_spacing_policy() -> Result<(), String> {
    let disabled = harfrust::Feature::new(CHWS, 0, ..);
    for (trim, explicit) in [
        (HanKerningTrim::SpaceAll, vec![]),
        (HanKerningTrim::Normal, vec![disabled]),
        (HanKerningTrim::Normal, vec![feature(b"halt")]),
        (HanKerningTrim::Normal, vec![feature(b"palt")]),
    ] {
        assert!(han_kerning_features(CHWS_FONT_BYTES, 0, CHWS_TEXT, trim, &explicit)?.is_empty());
    }
    let disabled_halt = harfrust::Feature::new(HALT, 0, ..);
    let features = han_kerning_features(
        CHWS_FONT_BYTES,
        0,
        CHWS_TEXT,
        HanKerningTrim::Normal,
        &[disabled_halt],
    )?;
    assert_full_range_chws(&features);
    Ok(())
}

#[test]
fn proportional_quotes_remain_narrow() {
    assert_eq!(quote_class('‘', false), HanClass::OpenNarrow);
    assert_eq!(quote_class('”', false), HanClass::CloseNarrow);
}

#[test]
fn fullwidth_quote_group_keeps_left_and_right_roles() {
    assert_eq!(quote_class('“', true), HanClass::Open);
    assert_eq!(quote_class('”', true), HanClass::Close);
}
