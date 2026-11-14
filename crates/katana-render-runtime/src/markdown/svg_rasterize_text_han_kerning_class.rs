use icu_properties::{
    CodePointMapData,
    props::{Block, EastAsianWidth, GeneralCategory},
};
use skrifa::{FontRef, MetadataProvider};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HanClass {
    Open,
    Close,
    OpenNarrow,
    CloseNarrow,
    Middle,
    Other,
}

const HALF_FACTOR: f32 = 2.0;
const HALF_WIDTH_RATIO: f32 = 0.5;

pub(super) fn classify(
    character: char,
    font: &FontRef<'_>,
    metrics: &skrifa::metrics::GlyphMetrics<'_>,
    quote_full: bool,
) -> HanClass {
    if matches!(character, '　' | '・' | '‧' | '･') {
        return HanClass::Middle;
    }
    let category = CodePointMapData::<GeneralCategory>::new().get(character);
    if !is_candidate(character, category) {
        return HanClass::Other;
    }
    let Some(glyph) = font.charmap().map(character) else {
        return HanClass::Other;
    };
    let Some(advance) = metrics.advance_width(glyph) else {
        return HanClass::Other;
    };
    let Some(bounds) = metrics.bounds(glyph) else {
        return HanClass::Other;
    };
    classify_bounds(character, category, advance, bounds, quote_full)
}

pub(super) fn is_candidate(character: char, category: GeneralCategory) -> bool {
    matches!(
        category,
        GeneralCategory::OpenPunctuation | GeneralCategory::ClosePunctuation
    ) || matches!(
        character,
        '、' | '。' | '，' | '．' | '：' | '；' | '‘' | '’' | '“' | '”'
    )
}

fn classify_bounds(
    character: char,
    category: GeneralCategory,
    advance: f32,
    bounds: skrifa::metrics::BoundingBox,
    quote_full: bool,
) -> HanClass {
    if !advance.is_finite() || advance <= 0.0 {
        return HanClass::Other;
    }
    if matches!(character, '‘' | '’' | '“' | '”') {
        return quote_class(character, quote_full);
    }
    let full = CodePointMapData::<Block>::new().get(character) == Block::CJKSymbolsAndPunctuation
        || CodePointMapData::<EastAsianWidth>::new().get(character) == EastAsianWidth::Fullwidth;
    if let Some(class) = bracket_class(category, full) {
        return class;
    }
    metric_class(advance, bounds).unwrap_or_else(|| middle_class(advance, bounds))
}

fn bracket_class(category: GeneralCategory, full: bool) -> Option<HanClass> {
    match (category, full) {
        (GeneralCategory::OpenPunctuation, true) => Some(HanClass::Open),
        (GeneralCategory::OpenPunctuation, false) => Some(HanClass::OpenNarrow),
        (GeneralCategory::ClosePunctuation, true) => Some(HanClass::Close),
        (GeneralCategory::ClosePunctuation, false) => Some(HanClass::CloseNarrow),
        _ => None,
    }
}

fn middle_class(advance: f32, bounds: skrifa::metrics::BoundingBox) -> HanClass {
    let half = advance / HALF_FACTOR;
    if bounds.x_max - bounds.x_min <= half && bounds.x_min >= half * HALF_WIDTH_RATIO {
        HanClass::Middle
    } else {
        HanClass::Other
    }
}

pub(super) fn quote_class(character: char, full: bool) -> HanClass {
    let open = matches!(character, '‘' | '“');
    if full {
        return if open {
            HanClass::Open
        } else {
            HanClass::Close
        };
    }
    if open {
        HanClass::OpenNarrow
    } else {
        HanClass::CloseNarrow
    }
}

pub(super) fn quote_is_fullwidth(
    font: &FontRef<'_>,
    metrics: &skrifa::metrics::GlyphMetrics<'_>,
) -> bool {
    quote_pair(font, metrics, ['‘', '“'], HanClass::Open)
        && quote_pair(font, metrics, ['’', '”'], HanClass::Close)
}

fn quote_pair(
    font: &FontRef<'_>,
    metrics: &skrifa::metrics::GlyphMetrics<'_>,
    chars: [char; 2],
    expected: HanClass,
) -> bool {
    let [Some(first), Some(second)] = chars.map(|character| font.charmap().map(character)) else {
        return false;
    };
    let (Some(first_advance), Some(second_advance)) =
        (metrics.advance_width(first), metrics.advance_width(second))
    else {
        return false;
    };
    let (Some(first_bounds), Some(second_bounds)) = (metrics.bounds(first), metrics.bounds(second))
    else {
        return false;
    };
    first_advance == second_advance
        && metric_class(first_advance, first_bounds) == Some(expected)
        && metric_class(second_advance, second_bounds) == Some(expected)
}

fn metric_class(advance: f32, bounds: skrifa::metrics::BoundingBox) -> Option<HanClass> {
    if !advance.is_finite() || advance <= 0.0 {
        return None;
    }
    let half = advance / HALF_FACTOR;
    if bounds.x_max <= half {
        Some(HanClass::Close)
    } else if bounds.x_min >= half {
        Some(HanClass::Open)
    } else {
        None
    }
}

pub(super) fn should_kern(current: HanClass, previous: HanClass) -> bool {
    matches!(current, HanClass::Open)
        && matches!(
            previous,
            HanClass::Open | HanClass::Middle | HanClass::Close | HanClass::OpenNarrow
        )
}

pub(super) fn should_kern_last(current: HanClass, previous: HanClass) -> bool {
    matches!(previous, HanClass::Close)
        && matches!(
            current,
            HanClass::Close | HanClass::CloseNarrow | HanClass::Middle
        )
}

#[cfg(test)]
#[path = "svg_rasterize_text_han_kerning_class_tests.rs"]
mod tests;
