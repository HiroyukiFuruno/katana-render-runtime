use super::class::{HanClass, quote_class, should_kern, should_kern_last};
use super::{HanKerningTrim, han_kerning_features};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

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
    let features = han_kerning_features(FONT_BYTES, 0, "plain ASCII", HanKerningTrim::Normal, &[])?;
    assert!(
        features
            .iter()
            .all(|feature| feature.tag != harfrust::Tag::from_be_bytes(*b"halt"))
    );
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
fn proportional_quotes_remain_narrow() {
    assert_eq!(quote_class('‘', false), HanClass::OpenNarrow);
    assert_eq!(quote_class('”', false), HanClass::CloseNarrow);
}

#[test]
fn fullwidth_quote_group_keeps_left_and_right_roles() {
    assert_eq!(quote_class('“', true), HanClass::Open);
    assert_eq!(quote_class('”', true), HanClass::Close);
}
