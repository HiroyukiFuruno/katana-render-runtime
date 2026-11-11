#[cfg(not(target_os = "windows"))]
use super::{
    MAX_CACHED_MEASUREMENTS, TextMeasurementCacheScope, TextMeasurementKey,
    persistent::{PersistentTextWidthCache, PersistentTextWidthKey},
};
use super::{PERSISTENT_TEXT_WIDTH_CACHE, text_width};
#[cfg(not(target_os = "windows"))]
use crate::markdown::svg_rasterize::HtmlTextMeasurementTokenKey;
use crate::markdown::svg_rasterize::{HtmlTextMeasurementToken, SvgRasterizeOps};
use crate::renderer::backends::html_interactive::style::CssStyle;

#[cfg(not(target_os = "windows"))]
const REPLACEMENT_WIDTH: f32 = 99.0;

fn platform_token(text: &str, family: &str) -> Result<Option<HtmlTextMeasurementToken>, String> {
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
        let key =
            PersistentTextWidthKey::new(TextMeasurementKey::new(&text, style), token_key.clone());
        last = Some(key.clone());
        cache.insert(key, index as f32);
    }
    last.map(|key| (cache, key))
        .ok_or("bounded cache did not receive a key".to_string())
}

#[test]
fn persistent_width_reuse_requires_render_scope_and_survives_frame_scope() -> Result<(), String> {
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
        cache.insert(last.clone(), REPLACEMENT_WIDTH);
        assert_eq!(cache.len(), MAX_CACHED_MEASUREMENTS);
        assert_eq!(cache.get(&last), Some(REPLACEMENT_WIDTH));
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
