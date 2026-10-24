use super::document::text_display_columns;
use super::style::{CssStyle, CssTextSpacingTrim};
use super::text_measurement_diagnostic::{
    record_lookup, record_text_total_timing, start_measurement, start_text_total_timing,
};
use crate::markdown::svg_rasterize::{HtmlTextMeasurementToken, SvgRasterizeOps, TextSpacingTrim};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashMap;

#[path = "text_width_persistent.rs"]
mod persistent;
use persistent::{PersistentTextWidthCache, PersistentTextWidthKey};

const MAX_CACHED_MEASUREMENTS: usize = 4096;
const SHA256_DIGEST_BYTES: usize = 32;

thread_local! {
    static TEXT_WIDTH_CACHE: RefCell<HashMap<TextMeasurementKey, f32>> = RefCell::new(HashMap::new());
    static PERSISTENT_TEXT_WIDTH_CACHE: RefCell<PersistentTextWidthCache> = RefCell::new(PersistentTextWidthCache::default());
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TextMeasurementKey {
    text_sha256: [u8; SHA256_DIGEST_BYTES],
    font_family_sha256: [u8; SHA256_DIGEST_BYTES],
    font_size: u32,
    font_weight: u16,
    italic: bool,
    letter_spacing: u32,
    font_feature_sha256: Option<[u8; SHA256_DIGEST_BYTES]>,
    text_spacing_trim: u8,
}

impl TextMeasurementKey {
    fn new(text: &str, style: &CssStyle) -> Self {
        Self {
            text_sha256: Sha256::digest(text.as_bytes()).into(),
            font_family_sha256: Sha256::digest(style.font_family.as_bytes()).into(),
            font_size: style.font_size.to_bits(),
            font_weight: style.font_weight,
            italic: style.italic,
            letter_spacing: style.letter_spacing.to_bits(),
            font_feature_sha256: style
                .font_feature_settings
                .as_deref()
                .map(|value| Sha256::digest(value.as_bytes()).into()),
            text_spacing_trim: match style.text_spacing_trim {
                CssTextSpacingTrim::Normal => 0,
                CssTextSpacingTrim::SpaceAll => 1,
            },
        }
    }
}

pub(super) struct TextMeasurementCacheScope;

impl TextMeasurementCacheScope {
    pub(super) fn enter() -> Self {
        clear_measurements();
        Self
    }
}

impl Drop for TextMeasurementCacheScope {
    fn drop(&mut self) {
        clear_measurements();
    }
}

fn clear_measurements() {
    SvgRasterizeOps::clear_html_line_metrics_cache();
    /* WHY: font資源を更新した次の描画へ旧幅を持ち越さず、描画後にcache容量も解放する。 */
    TEXT_WIDTH_CACHE.with(|cache| *cache.borrow_mut() = HashMap::new());
}

pub(super) fn text_width(text: &str, style: &CssStyle) -> f32 {
    let started = start_text_total_timing();
    let width = text_width_body(text, style);
    record_text_total_timing(started);
    width
}

fn text_width_body(text: &str, style: &CssStyle) -> f32 {
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
    let prepared = SvgRasterizeOps::prepare_html_text_measurement_with_trim(
        text,
        &style.font_family,
        style.font_size,
        (style.font_weight, style.italic),
        style.letter_spacing,
        style.font_feature_settings.as_deref(),
        match style.text_spacing_trim {
            CssTextSpacingTrim::Normal => TextSpacingTrim::Normal,
            CssTextSpacingTrim::SpaceAll => TextSpacingTrim::SpaceAll,
        },
    );
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

#[cfg(test)]
fn measured_text_width(text: &str, style: &CssStyle) -> f32 {
    measured_text_width_with_status(text, style).0
}

#[cfg(test)]
fn measured_text_width_with_status(text: &str, style: &CssStyle) -> (f32, bool) {
    match SvgRasterizeOps::measure_html_text_with_cache_status(
        text,
        &style.font_family,
        style.font_size,
        style.font_weight,
        style.italic,
        style.letter_spacing,
        style.font_feature_settings.as_deref(),
    ) {
        Ok(result) => result,
        Err(_) => (heuristic_text_width(text, style), false),
    }
}

fn heuristic_text_width(text: &str, style: &CssStyle) -> f32 {
    let spacing = text.chars().count().saturating_sub(1) as f32 * style.letter_spacing;
    text_display_columns(text) as f32 * style.font_size * style.text_character_width_factor()
        + spacing
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_CACHED_MEASUREMENTS, PERSISTENT_TEXT_WIDTH_CACHE, TEXT_WIDTH_CACHE,
        TextMeasurementCacheScope, TextMeasurementKey, heuristic_text_width,
        insert_bounded_measurement, measured_text_width, text_width,
    };
    #[cfg(not(target_os = "windows"))]
    use super::{persistent::PersistentTextWidthCache, persistent::PersistentTextWidthKey};
    #[cfg(not(target_os = "windows"))]
    use crate::markdown::svg_rasterize::HtmlTextMeasurementTokenKey;
    use crate::markdown::svg_rasterize::{HtmlTextMeasurementToken, SvgRasterizeOps};
    use crate::renderer::backends::html_interactive::style::CssStyle;
    use std::collections::HashMap;

    fn platform_token(
        text: &str,
        family: &str,
    ) -> Result<Option<HtmlTextMeasurementToken>, String> {
        let (token, usable) = SvgRasterizeOps::with_html_font_render_scope(|| {
            let token = SvgRasterizeOps::html_text_measurement_token(text, family);
            let usable = token.as_ref().is_some_and(|token| token.is_usable());
            (token, usable)
        });
        #[cfg(target_os = "windows")]
        {
            if token.is_some() || usable {
                return Err("Windows token policy requires no durable token".to_string());
            }
            return Ok(None);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let token = token.ok_or("render scope token missing".to_string())?;
            if !usable {
                return Err("render scope token is not usable".to_string());
            }
            Ok(Some(token))
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn render_frame_width(text: &str, style: &CssStyle) -> f32 {
        SvgRasterizeOps::with_html_font_render_scope(|| {
            let _scope = TextMeasurementCacheScope::enter();
            text_width(text, style)
        })
    }

    #[cfg(not(target_os = "windows"))]
    fn build_bounded_cache(
        token_key: HtmlTextMeasurementTokenKey,
        style: &CssStyle,
    ) -> Result<(PersistentTextWidthCache, PersistentTextWidthKey), String> {
        let mut cache = PersistentTextWidthCache::default();
        let mut last = None;
        for index in 0..=MAX_CACHED_MEASUREMENTS {
            let text = format!("persistent-bound-{index}");
            let key = PersistentTextWidthKey::new(
                TextMeasurementKey::new(&text, style),
                token_key.clone(),
            );
            last = Some(key.clone());
            cache.insert(key, index as f32);
        }
        last.map(|key| (cache, key))
            .ok_or("bounded cache did not receive a key".to_string())
    }

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
    fn measurement_status_matches_legacy_width() -> Result<(), String> {
        let style = CssStyle::browser_default();
        let legacy = SvgRasterizeOps::measure_html_text(
            "status probe",
            &style.font_family,
            style.font_size,
            style.font_weight,
            style.italic,
            style.letter_spacing,
            style.font_feature_settings.as_deref(),
        );
        let status = SvgRasterizeOps::measure_html_text_with_cache_status(
            "status probe",
            &style.font_family,
            style.font_size,
            style.font_weight,
            style.italic,
            style.letter_spacing,
            style.font_feature_settings.as_deref(),
        );
        let expected = legacy.map_err(legacy_measurement_error)?;
        let (actual, shaped) = status.map_err(status_measurement_error)?;
        assert!(shaped);
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn measurement_status_marks_empty_and_fallback() -> Result<(), String> {
        let style = CssStyle::browser_default();
        let empty = SvgRasterizeOps::measure_html_text_with_cache_status(
            "",
            &style.font_family,
            style.font_size,
            style.font_weight,
            style.italic,
            style.letter_spacing,
            style.font_feature_settings.as_deref(),
        );
        assert!(matches!(empty, Ok((0.0, false))));

        let fallback = SvgRasterizeOps::measure_html_text_with_cache_status(
            "fallback",
            "Definitely Missing Font",
            style.font_size,
            style.font_weight,
            style.italic,
            style.letter_spacing,
            style.font_feature_settings.as_deref(),
        );
        let (_, shaped) = fallback.map_err(fallback_measurement_error)?;
        assert!(!shaped);
        Ok(())
    }

    #[test]
    fn persistent_width_reuse_requires_render_scope_and_survives_frame_scope() -> Result<(), String>
    {
        let style = CssStyle::browser_default();
        let text = "persistent width scope probe";
        PERSISTENT_TEXT_WIDTH_CACHE.with(|cache| cache.borrow_mut().clear_for_test());
        let outside = text_width(text, &style);
        assert!(outside > 0.0);
        PERSISTENT_TEXT_WIDTH_CACHE.with(|cache| assert_eq!(cache.borrow().len(), 0));

        let token = platform_token(text, &style.font_family)?;
        #[cfg(target_os = "windows")]
        {
            assert!(
                token.is_none(),
                "Windows token policy requires no durable token"
            );
        }
        #[cfg(not(target_os = "windows"))]
        {
            let token = token.ok_or("render scope token missing")?;
            let first = render_frame_width(text, &style);
            PERSISTENT_TEXT_WIDTH_CACHE.with(|cache| assert_eq!(cache.borrow().len(), 1));
            let second = render_frame_width(text, &style);
            assert_eq!(first, second);
            PERSISTENT_TEXT_WIDTH_CACHE.with(|cache| assert_eq!(cache.borrow().len(), 1));
            assert!(!token.is_usable());
        }
        Ok(())
    }

    #[test]
    fn persistent_width_cache_is_bounded_and_updates_existing_keys() -> Result<(), String> {
        let style = CssStyle::browser_default();
        let token = platform_token("persistent-bound-token", &style.font_family)?;
        #[cfg(target_os = "windows")]
        {
            assert!(
                token.is_none(),
                "Windows token policy requires no durable token"
            );
        }
        #[cfg(not(target_os = "windows"))]
        {
            let token = token.ok_or("render scope token missing")?;
            let (mut cache, last) = build_bounded_cache(token.key(), &style)?;
            assert_eq!(cache.len(), MAX_CACHED_MEASUREMENTS);
            cache.insert(last.clone(), 99.0);
            assert_eq!(cache.len(), MAX_CACHED_MEASUREMENTS);
            assert_eq!(cache.get(&last), Some(99.0));
        }
        Ok(())
    }

    #[test]
    fn measurement_token_key_tracks_proof_identity() -> Result<(), String> {
        let style = CssStyle::browser_default();
        let first = platform_token("token identity A", &style.font_family)?;
        #[cfg(target_os = "windows")]
        {
            assert!(
                first.is_none(),
                "Windows token policy requires no durable token"
            );
        }
        #[cfg(not(target_os = "windows"))]
        {
            let first = first.ok_or("render scope token missing")?;
            let same = platform_token("token identity B", &style.font_family)?
                .ok_or("same-request token missing")?;
            let other = platform_token("token identity A", "Noto Sans")?
                .ok_or("alternate-request token missing")?;
            assert_eq!(first.key(), same.key());
            assert_ne!(first.key(), other.key());
        }
        Ok(())
    }

    #[test]
    fn render_scope_discards_previous_widths_and_releases_cache_storage() {
        let style = CssStyle::browser_default();
        assert!(text_width("previous document", &style) > 0.0);
        {
            let _scope = TextMeasurementCacheScope::enter();
            TEXT_WIDTH_CACHE.with(|cache| assert!(cache.borrow().is_empty()));
            assert!(text_width("current document", &style) > 0.0);
            TEXT_WIDTH_CACHE.with(|cache| assert!(!cache.borrow().is_empty()));
        }
        TEXT_WIDTH_CACHE.with(|cache| {
            assert!(cache.borrow().is_empty());
            assert_eq!(cache.borrow().capacity(), 0);
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

    #[cfg(target_os = "macos")]
    #[test]
    fn japanese_paragraph_kerning_matches_chrome_dom_measurement() {
        let mut style = CssStyle::browser_default();
        style.font_family =
            "-apple-system, BlinkMacSystemFont, \"Hiragino Sans\", sans-serif".into();
        style.font_size = 16.0;
        let cases = [
            (
                "検索結果は通信環境や蔵書の更新状況によって表示順が変わる場合があり",
                527.531_25,
            ),
            (
                "ます。利用者が操作を繰り返しても、現在の検索条件と結果件数を画面上部",
                541.453_1,
            ),
            (
                "資料に付属するメディアや参考資料がある場合は、詳細欄にまとまりとして",
                540.812_5,
            ),
        ];
        for (text, expected) in cases {
            let default = measured_text_width(text, &style);
            style.font_feature_settings = Some("'kern' 1".into());
            let explicit = measured_text_width(text, &style);
            style.font_feature_settings = None;
            assert!(
                (default - expected).abs() <= 1.0 / 64.0,
                "Chrome DOM={expected}, default={default}, explicit kern={explicit}",
            );
        }
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

    fn legacy_measurement_error(
        error: crate::markdown::svg_rasterize::SvgRasterizeError,
    ) -> String {
        format!("legacy measurement failed: {error}")
    }

    fn status_measurement_error(
        error: crate::markdown::svg_rasterize::SvgRasterizeError,
    ) -> String {
        format!("status measurement failed: {error}")
    }

    fn fallback_measurement_error(
        error: crate::markdown::svg_rasterize::SvgRasterizeError,
    ) -> String {
        format!("fallback measurement failed: {error}")
    }

    #[test]
    fn measurement_errors_preserve_real_svg_parse_failure() -> Result<(), String> {
        let error = SvgRasterizeOps::rasterize_svg("<", 1.0)
            .err()
            .ok_or("SVG parse failure missing")?;
        let expected = error.to_string();
        assert_eq!(
            legacy_measurement_error(error),
            format!("legacy measurement failed: {expected}")
        );
        let status_error = SvgRasterizeOps::rasterize_svg("<", 1.0)
            .err()
            .ok_or("SVG parse failure missing")?;
        assert_eq!(
            status_measurement_error(status_error),
            format!("status measurement failed: {expected}")
        );
        let fallback_error = SvgRasterizeOps::rasterize_svg("<", 1.0)
            .err()
            .ok_or("SVG parse failure missing")?;
        assert_eq!(
            fallback_measurement_error(fallback_error),
            format!("fallback measurement failed: {expected}")
        );
        Ok(())
    }
}
