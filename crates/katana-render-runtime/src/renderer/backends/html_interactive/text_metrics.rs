use super::document::text_display_columns;
use super::style::CssStyle;
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use std::cell::RefCell;
use std::collections::HashMap;

const MAX_CACHED_MEASUREMENTS: usize = 4096;

thread_local! {
    static TEXT_WIDTH_CACHE: RefCell<HashMap<TextMeasurementKey, f32>> = RefCell::new(HashMap::new());
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TextMeasurementKey {
    text: String,
    font_family: String,
    font_size: u32,
    font_weight: u16,
    italic: bool,
    letter_spacing: u32,
    font_feature_settings: Option<String>,
}

impl TextMeasurementKey {
    fn new(text: &str, style: &CssStyle) -> Self {
        Self {
            text: text.to_string(),
            font_family: style.font_family.clone(),
            font_size: style.font_size.to_bits(),
            font_weight: style.font_weight,
            italic: style.italic,
            letter_spacing: style.letter_spacing.to_bits(),
            font_feature_settings: style.font_feature_settings.clone(),
        }
    }
}

pub(super) fn text_width(text: &str, style: &CssStyle) -> f32 {
    let transformed = style.transformed_text(text);
    if transformed.is_empty() {
        return 0.0;
    }
    let key = TextMeasurementKey::new(&transformed, style);
    TEXT_WIDTH_CACHE.with(|cache| {
        if let Some(width) = cache.borrow().get(&key) {
            return *width;
        }
        let width = measured_text_width(&transformed, style);
        insert_bounded_measurement(&mut cache.borrow_mut(), key, width, MAX_CACHED_MEASUREMENTS);
        width
    })
}

fn insert_bounded_measurement(
    cache: &mut HashMap<TextMeasurementKey, f32>,
    key: TextMeasurementKey,
    width: f32,
    maximum_entries: usize,
) {
    if cache.len() >= maximum_entries
        && let Some(evicted_key) = cache.keys().next().cloned()
    {
        cache.remove(&evicted_key);
    }
    cache.insert(key, width);
}

fn measured_text_width(text: &str, style: &CssStyle) -> f32 {
    SvgRasterizeOps::measure_html_text(
        text,
        &style.font_family,
        style.font_size,
        style.font_weight,
        style.italic,
        style.letter_spacing,
        style.font_feature_settings.as_deref(),
    )
    .unwrap_or_else(|_| heuristic_text_width(text, style))
}

fn heuristic_text_width(text: &str, style: &CssStyle) -> f32 {
    let spacing = text.chars().count().saturating_sub(1) as f32 * style.letter_spacing;
    text_display_columns(text) as f32 * style.font_size * style.text_character_width_factor()
        + spacing
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_CACHED_MEASUREMENTS, TEXT_WIDTH_CACHE, TextMeasurementKey, heuristic_text_width,
        insert_bounded_measurement, measured_text_width, text_width,
    };
    use crate::renderer::backends::html_interactive::style::CssStyle;
    use std::collections::HashMap;

    #[test]
    fn text_measurement_handles_empty_cached_and_bounded_paths() {
        let style = CssStyle::browser_default();
        assert_eq!(text_width("", &style), 0.0);
        let first = text_width("cached", &style);
        assert_eq!(text_width("cached", &style), first);

        for index in 0..8 {
            assert!(text_width(&format!("entry-{index}"), &style) > 0.0);
        }
        TEXT_WIDTH_CACHE.with(|cache| {
            assert!(cache.borrow().len() <= MAX_CACHED_MEASUREMENTS);
        });
    }

    #[test]
    fn bounded_cache_eviction_preserves_capacity_instead_of_clearing() {
        let style = CssStyle::browser_default();
        let mut cache = HashMap::new();
        insert_bounded_measurement(&mut cache, TextMeasurementKey::new("first", &style), 1.0, 2);
        insert_bounded_measurement(
            &mut cache,
            TextMeasurementKey::new("second", &style),
            2.0,
            2,
        );
        insert_bounded_measurement(&mut cache, TextMeasurementKey::new("third", &style), 3.0, 2);

        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn unknown_font_family_uses_rasterizer_fallback_measurement() {
        let mut style = CssStyle::browser_default();
        style.font_family = "Definitely Missing Font".to_string();
        assert!(text_width("fallback", &style) > 0.0);
    }

    #[test]
    fn heuristic_width_accounts_for_display_columns_and_letter_spacing() {
        let plain = CssStyle::browser_default();
        let mut spaced = plain.clone();
        spaced.letter_spacing = 2.0;

        assert!(heuristic_text_width("日本 A", &spaced) > heuristic_text_width("日本 A", &plain));
    }

    #[test]
    fn invalid_fallback_svg_uses_deterministic_heuristic_width() {
        let mut style = CssStyle::browser_default();
        style.font_family = "Definitely Missing Font".to_string();

        assert_eq!(
            measured_text_width("\0", &style),
            heuristic_text_width("\0", &style)
        );
    }
}
