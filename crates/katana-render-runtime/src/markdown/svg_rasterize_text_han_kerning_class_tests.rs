use super::{HanClass, classify, classify_bounds, quote_is_fullwidth};
use icu_properties::{CodePointMapData, props::GeneralCategory};
use skrifa::{FontRef, MetadataProvider, instance::LocationRef, prelude::Size};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

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
