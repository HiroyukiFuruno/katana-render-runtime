use super::super::style::{CssStyle, CssTextSpacingTrim};
use super::super::text_measurement_diagnostic::{
    record_lookup, record_preparation_timing, start_measurement, start_preparation_timing,
};
use super::persistent::PersistentTextWidthKey;
use super::{
    MAX_CACHED_MEASUREMENTS, PERSISTENT_TEXT_WIDTH_CACHE, TEXT_WIDTH_CACHE, TextMeasurementKey,
    heuristic_text_width, insert_bounded_measurement,
};
use crate::markdown::svg_rasterize::{
    HtmlTextMeasurementToken, PreparedHtmlTextMeasurement, SvgRasterizeOps, TextSpacingTrim,
};

pub(super) fn text_width_body(text: &str, style: &CssStyle) -> f32 {
    let transformed = style.transformed_text(text);
    if transformed.is_empty() {
        return 0.0;
    }
    let key = TextMeasurementKey::new(&transformed, style);
    if let Some(width) = TEXT_WIDTH_CACHE.with(|cache| cache.borrow().get(&key).copied()) {
        record_lookup(true, None);
        return width;
    }
    text_width_after_cache_miss(&transformed, style, key)
}

fn text_width_after_cache_miss(text: &str, style: &CssStyle, key: TextMeasurementKey) -> f32 {
    let preparation_started = start_preparation_timing();
    let trim = text_spacing_trim(style);
    if let Some(width) = native_width(text, style, trim) {
        record_preparation_timing(preparation_started);
        record_lookup(false, None);
        cache_ephemeral_width(key, width);
        return width;
    }
    let prepared = prepare_width(text, style, trim);
    record_preparation_timing(preparation_started);
    use_prepared_width(text, style, key, prepared)
}

fn text_spacing_trim(style: &CssStyle) -> TextSpacingTrim {
    match style.text_spacing_trim {
        CssTextSpacingTrim::Normal => TextSpacingTrim::Normal,
        CssTextSpacingTrim::SpaceAll => TextSpacingTrim::SpaceAll,
    }
}

fn native_width(text: &str, style: &CssStyle, trim: TextSpacingTrim) -> Option<f32> {
    SvgRasterizeOps::native_html_text_advance(
        text,
        style.raster_font_style(),
        style.letter_spacing,
        style.font_feature_settings.as_deref(),
        trim,
    )
}

fn prepare_width<'a>(
    text: &'a str,
    style: &'a CssStyle,
    trim: TextSpacingTrim,
) -> PreparedHtmlTextMeasurement<'a> {
    SvgRasterizeOps::prepare_html_text_measurement_with_trim(
        text,
        style.used_font_family(),
        style.font_size,
        (style.font_weight, style.italic),
        style.letter_spacing,
        style.font_feature_settings.as_deref(),
        trim,
    )
}

fn use_prepared_width(
    text: &str,
    style: &CssStyle,
    key: TextMeasurementKey,
    prepared: PreparedHtmlTextMeasurement<'_>,
) -> f32 {
    let token = prepared.token();
    if let Some(width) = persistent_width(&key, token) {
        record_lookup(false, None);
        cache_ephemeral_width(key, width);
        return width;
    }
    let measured_started = start_measurement();
    let (width, shaping_succeeded) = prepared
        .measure_with_cache_status()
        .unwrap_or_else(|_| (heuristic_text_width(text, style), false));
    record_lookup(false, measured_started);
    cache_ephemeral_width(key.clone(), width);
    persist_shaped_width(key, token, width, shaping_succeeded);
    width
}

fn persistent_width(
    key: &TextMeasurementKey,
    token: Option<&HtmlTextMeasurementToken>,
) -> Option<f32> {
    let token = token.filter(|token| token.is_usable())?;
    let persistent_key = PersistentTextWidthKey::new(key.clone(), token.key());
    PERSISTENT_TEXT_WIDTH_CACHE.with(|cache| cache.borrow().get(&persistent_key))
}

fn cache_ephemeral_width(key: TextMeasurementKey, width: f32) {
    TEXT_WIDTH_CACHE.with(|cache| {
        insert_bounded_measurement(&mut cache.borrow_mut(), key, width, MAX_CACHED_MEASUREMENTS);
    });
}

fn persist_shaped_width(
    key: TextMeasurementKey,
    token: Option<&HtmlTextMeasurementToken>,
    width: f32,
    shaping_succeeded: bool,
) {
    if !shaping_succeeded {
        return;
    }
    let Some(token) = token.filter(|token| token.is_usable()) else {
        return;
    };
    let persistent_key = PersistentTextWidthKey::new(key, token.key());
    PERSISTENT_TEXT_WIDTH_CACHE.with(|cache| cache.borrow_mut().insert(persistent_key, width));
}
