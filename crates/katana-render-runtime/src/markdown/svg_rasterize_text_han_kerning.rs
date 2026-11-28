use icu_properties::{CodePointMapData, props::GeneralCategory};
use skrifa::{FontRef, MetadataProvider, Tag, prelude::Size, raw::TableProvider};

#[path = "svg_rasterize_text_han_kerning_class.rs"]
mod class;
use class::{HanClass, classify, is_candidate, quote_is_fullwidth, should_kern, should_kern_last};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HanKerningTrim {
    Normal,
    SpaceAll,
}

const CHWS: Tag = Tag::new(b"chws");
const HALT: Tag = Tag::new(b"halt");
const PALT: Tag = Tag::new(b"palt");

/* WHY: 既存shaperがUTF-8 byte offsetをclusterとして生成する契約に合わせる。 */
pub(super) fn han_kerning_features(
    bytes: &[u8],
    face_index: u32,
    text: &str,
    trim: HanKerningTrim,
    explicit: &[harfrust::Feature],
) -> Result<Vec<harfrust::Feature>, String> {
    if !has_han_candidate(text) {
        return Ok(Vec::new());
    }
    let font = FontRef::from_index(bytes, face_index).map_err(|error| error.to_string())?;
    if trim == HanKerningTrim::SpaceAll || has_exclusive(explicit) {
        return Ok(Vec::new());
    }
    if has_layout_feature(&font, CHWS) {
        return Ok(vec![harfrust::Feature::new(CHWS, 1, ..)]);
    }
    if !has_layout_feature(&font, HALT) {
        return Ok(Vec::new());
    }
    let location = font.axes().location(std::iter::empty::<(&str, f32)>());
    let metrics = font.glyph_metrics(Size::unscaled(), &location);
    Ok(halt_ranges(
        text,
        &font,
        &metrics,
        quote_is_fullwidth(&font, &metrics),
    ))
}

pub(in super::super) fn has_han_candidate(text: &str) -> bool {
    text.chars().any(|character| {
        if matches!(character, '　' | '・' | '‧' | '･') {
            return true;
        }
        is_candidate(
            character,
            CodePointMapData::<GeneralCategory>::new().get(character),
        )
    })
}

fn has_exclusive(features: &[harfrust::Feature]) -> bool {
    features.iter().any(|feature| {
        feature.tag == CHWS || (feature.value != 0 && matches!(feature.tag, HALT | PALT))
    })
}

fn has_layout_feature(font: &FontRef<'_>, wanted: Tag) -> bool {
    [
        font.gsub().ok().and_then(|table| table.feature_list().ok()),
        font.gpos().ok().and_then(|table| table.feature_list().ok()),
    ]
    .into_iter()
    .flatten()
    .any(|list| {
        list.feature_records()
            .iter()
            .any(|record| record.feature_tag() == wanted)
    })
}

fn halt_ranges(
    text: &str,
    font: &FontRef<'_>,
    metrics: &skrifa::metrics::GlyphMetrics<'_>,
    quote_full: bool,
) -> Vec<harfrust::Feature> {
    let chars = classified_characters(text, font, metrics, quote_full);
    let mut ranges = Vec::new();
    for (index, &(byte, character, current)) in chars.iter().enumerate() {
        let Some(&(previous_byte, _, previous)) = index.checked_sub(1).and_then(|i| chars.get(i))
        else {
            continue;
        };
        if should_kern(current, previous) {
            ranges.push(harfrust::Feature::new(
                HALT,
                1,
                byte..byte + character.len_utf8(),
            ));
        } else if should_kern_last(current, previous) {
            ranges.push(harfrust::Feature::new(HALT, 1, previous_byte..byte));
        }
    }
    ranges
}

fn classified_characters(
    text: &str,
    font: &FontRef<'_>,
    metrics: &skrifa::metrics::GlyphMetrics<'_>,
    quote_full: bool,
) -> Vec<(usize, char, HanClass)> {
    text.char_indices()
        .map(|(byte, character)| {
            (
                byte,
                character,
                classify(character, font, metrics, quote_full),
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "svg_rasterize_text_han_kerning_tests.rs"]
mod tests;
