use super::{HanClass, classify, classify_bounds, quote_is_fullwidth};
use crate::markdown::svg_rasterize::text_shaping::test_font_data::font_with_empty_table;
use icu_properties::{CodePointMapData, props::GeneralCategory};
use rustybuzz::ttf_parser::Tag;
use skrifa::{FontRef, MetadataProvider, instance::LocationRef, prelude::Size};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const CJK_FONT_BYTES: &[u8] =
    include_bytes!("../../../../tests/fixtures/fonts/han-kerning/NotoSansCJKjp-han14.otf");

fn font_metrics() -> Result<(FontRef<'static>, skrifa::metrics::GlyphMetrics<'static>), String> {
    let font = FontRef::from_index(FONT_BYTES, 0).map_err(|error| error.to_string())?;
    let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
    Ok((font, metrics))
}

fn assert_invalid_advance_is_other(
    bounds: skrifa::metrics::BoundingBox,
    category: GeneralCategory,
) {
    assert_eq!(
        classify_bounds('（', category, 0.0, bounds, false),
        HanClass::Other
    );
    assert_eq!(
        classify_bounds('（', category, f32::NAN, bounds, false),
        HanClass::Other
    );
}

#[test]
fn unicode_fullwidth_bracket_categories_select_roles_with_valid_font_metrics() -> Result<(), String>
{
    let (font, metrics) = font_metrics()?;
    let glyph = font
        .charmap()
        .map('A')
        .ok_or("bundled font has no ASCII A glyph")?;
    let advance = metrics
        .advance_width(glyph)
        .ok_or("ASCII A advance unavailable")?;
    let bounds = metrics.bounds(glyph).ok_or("ASCII A bounds unavailable")?;
    let cases = [('（', HanClass::Open), ('）', HanClass::Close)];

    /* WHY: CJK glyphのないfixtureでもUnicode category政策を有効metricsで検査する。 */
    for (character, expected) in cases {
        let category = CodePointMapData::<GeneralCategory>::new().get(character);
        assert!(super::is_candidate(character, category));
        assert_eq!(
            classify_bounds(character, category, advance, bounds, false),
            expected
        );
    }
    Ok(())
}

#[test]
fn bundled_font_classifies_available_narrow_brackets_through_real_glyphs() -> Result<(), String> {
    let (font, metrics) = font_metrics()?;

    assert_eq!(classify('(', &font, &metrics, false), HanClass::OpenNarrow);
    assert_eq!(classify(')', &font, &metrics, false), HanClass::CloseNarrow);
    Ok(())
}

#[test]
fn bundled_non_cjk_font_missing_fullwidth_glyph_falls_back_to_other() -> Result<(), String> {
    let (font, metrics) = font_metrics()?;

    assert!(font.charmap().map('（').is_none());
    assert_eq!(classify('（', &font, &metrics, false), HanClass::Other);
    Ok(())
}

#[test]
fn bundled_font_quote_width_controls_open_and_close_roles() -> Result<(), String> {
    let (font, metrics) = font_metrics()?;
    let full = quote_is_fullwidth(&font, &metrics);
    assert!(!full, "bundled Noto Sans quotes should remain proportional");
    let cases = [
        ('‘', HanClass::OpenNarrow),
        ('’', HanClass::CloseNarrow),
        ('“', HanClass::OpenNarrow),
        ('”', HanClass::CloseNarrow),
    ];

    for (character, expected) in cases {
        let glyph = font
            .charmap()
            .map(character)
            .ok_or("bundled font quote glyph unavailable")?;
        assert!(metrics.advance_width(glyph).is_some());
        assert_eq!(classify(character, &font, &metrics, full), expected);
    }
    Ok(())
}

#[test]
fn punctuation_middle_missing_glyph_and_invalid_advance_follow_fallbacks() -> Result<(), String> {
    let (font, metrics) = font_metrics()?;

    for character in ['　', '・', '‧', '･'] {
        assert_eq!(
            classify(character, &font, &metrics, false),
            HanClass::Middle
        );
    }
    assert_eq!(classify('A', &font, &metrics, false), HanClass::Other);
    assert_eq!(
        classify('\u{10ffff}', &font, &metrics, false),
        HanClass::Other
    );
    let category = CodePointMapData::<GeneralCategory>::new().get('（');
    let glyph = font
        .charmap()
        .map('A')
        .ok_or("bundled font has no ASCII A glyph")?;
    let bounds = metrics.bounds(glyph).ok_or("ASCII A bounds unavailable")?;
    assert_invalid_advance_is_other(bounds, category);
    Ok(())
}

fn bounds(x_min: f32, x_max: f32) -> skrifa::metrics::BoundingBox {
    skrifa::metrics::BoundingBox {
        x_min,
        y_min: 0.0,
        x_max,
        y_max: 1.0,
    }
}

#[test]
fn metric_class_handles_half_advance_boundaries_and_invalid_advances() {
    let cases = [
        (10.0, bounds(1.0, 5.0), Some(HanClass::Close)),
        (10.0, bounds(5.0, 9.0), Some(HanClass::Open)),
        (10.0, bounds(4.0, 6.0), None),
        (0.0, bounds(1.0, 2.0), None),
        (f32::NAN, bounds(1.0, 2.0), None),
        (f32::INFINITY, bounds(1.0, 2.0), None),
    ];
    for (advance, glyph_bounds, expected) in cases {
        assert_eq!(super::metric_class(advance, glyph_bounds), expected);
    }
}

#[test]
fn centered_punctuation_uses_middle_only_within_width_and_position_limits() {
    let category = GeneralCategory::OtherPunctuation;
    let cases = [
        (bounds(2.5, 7.5), HanClass::Middle),
        (bounds(2.4, 7.4), HanClass::Other),
        (bounds(2.5, 7.6), HanClass::Other),
    ];
    for (glyph_bounds, expected) in cases {
        assert_eq!(
            classify_bounds('、', category, 10.0, glyph_bounds, false),
            expected
        );
    }
}

#[test]
fn quote_pair_rejects_missing_glyph_and_mismatched_roles() -> Result<(), String> {
    let (font, metrics) = font_metrics()?;
    assert!(!super::quote_pair(
        &font,
        &metrics,
        ['(', '\u{10ffff}'],
        HanClass::Open
    ));
    for character in ['(', ')', '[', ']'] {
        let glyph = font
            .charmap()
            .map(character)
            .ok_or("bracket glyph unavailable")?;
        let advance = metrics
            .advance_width(glyph)
            .ok_or("bracket advance unavailable")?;
        let glyph_bounds = metrics.bounds(glyph).ok_or("bracket bounds unavailable")?;
        for expected in [HanClass::Open, HanClass::Close] {
            assert_eq!(
                super::quote_pair(&font, &metrics, [character, character], expected),
                super::metric_class(advance, glyph_bounds) == Some(expected)
            );
        }
    }
    assert_invalid_font_tables()
}

fn assert_invalid_font_tables() -> Result<(), String> {
    for (tag, has_advance) in [
        (Tag::from_bytes(b"maxp"), false),
        (Tag::from_bytes(b"glyf"), true),
    ] {
        let font_bytes = font_with_empty_table(FONT_BYTES, tag)?;
        assert_unusable_punctuation_metrics(&font_bytes, has_advance)?;
    }
    Ok(())
}

fn assert_unusable_punctuation_metrics(data: &[u8], has_advance: bool) -> Result<(), String> {
    let font = FontRef::from_index(data, 0).map_err(|error| error.to_string())?;
    let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
    let glyph = font
        .charmap()
        .map('(')
        .ok_or("bracket cmap entry missing")?;
    assert_eq!(metrics.advance_width(glyph).is_some(), has_advance);
    if has_advance {
        assert!(metrics.bounds(glyph).is_none());
    }
    assert_eq!(classify('(', &font, &metrics, false), HanClass::Other);
    assert!(!super::quote_pair(
        &font,
        &metrics,
        ['(', '('],
        HanClass::Open
    ));
    Ok(())
}

#[test]
fn cjk_quote_pair_checks_both_glyph_roles_with_equal_advances() -> Result<(), String> {
    let font = FontRef::from_index(CJK_FONT_BYTES, 0).map_err(|error| error.to_string())?;
    let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
    let open = font.charmap().map('「').ok_or("open glyph missing")?;
    let close = font.charmap().map('」').ok_or("close glyph missing")?;
    let open_advance = metrics.advance_width(open).ok_or("open advance missing")?;
    assert!(open_advance > 0.0);
    assert_eq!(metrics.advance_width(close), Some(open_advance));
    for (characters, role, expected) in [
        (['「', '「'], HanClass::Open, true),
        (['」', '」'], HanClass::Close, true),
        (['「', '」'], HanClass::Open, false),
    ] {
        assert_eq!(
            super::quote_pair(&font, &metrics, characters, role),
            expected
        );
    }
    Ok(())
}
