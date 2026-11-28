use super::super::style::CssStyle;
use crate::markdown::svg_rasterize::{HtmlPaintFontRun, SvgRasterizeOps};
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
};

/* WHY: 同一行の再描画でfallback走査を繰り返さず、入力数に応じたメモリ増加を制限する。 */
const MAX_CACHED_FONT_MARKUPS: usize = 4096;
const MAX_CACHED_FONT_MARKUP_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct FontMarkupKey {
    text: String,
    font_family: String,
    language: Option<String>,
    font_weight: u16,
    italic: bool,
    x: u32,
    font_size: u32,
    letter_spacing: u32,
    features: Option<String>,
}

#[derive(Default)]
pub(crate) struct FontMarkupCache {
    entries: HashMap<FontMarkupKey, (String, String)>,
    retained_bytes: usize,
}

impl FontMarkupCache {
    #[cfg(test)]
    fn font_markup(&mut self, text: &str, style: &CssStyle) -> (String, String) {
        self.font_markup_at(text, style, 0.0)
    }

    pub(in super::super) fn font_markup_at(
        &mut self,
        text: &str,
        style: &CssStyle,
        x: f32,
    ) -> (String, String) {
        let key = FontMarkupKey {
            text: text.to_string(),
            font_family: style.used_font_family().to_owned(),
            language: style.effective_font_language().map(str::to_owned),
            font_weight: style.font_weight,
            italic: style.italic,
            x: x.to_bits(),
            font_size: style.font_size.to_bits(),
            letter_spacing: style.letter_spacing.to_bits(),
            features: style.font_feature_settings.clone(),
        };
        if let Some(markup) = self.entries.get(&key).cloned() {
            return markup;
        }
        let markup = resolved_font_markup(text, style, x);
        self.cache_markup(key, markup.clone());
        markup
    }

    fn cache_markup(&mut self, key: FontMarkupKey, markup: (String, String)) {
        let bytes = retained_markup_bytes(&key, &markup);
        if bytes > MAX_CACHED_FONT_MARKUP_BYTES {
            return;
        }
        while let Some(evicted_key) = self.entries.keys().next().cloned() {
            if self.entries.len() < MAX_CACHED_FONT_MARKUPS
                && self.retained_bytes + bytes <= MAX_CACHED_FONT_MARKUP_BYTES
            {
                break;
            }
            if let Some((evicted_key, evicted)) = self.entries.remove_entry(&evicted_key) {
                self.retained_bytes -= retained_markup_bytes(&evicted_key, &evicted);
            }
        }
        self.retained_bytes += bytes;
        self.entries.insert(key, markup);
    }
}

fn retained_markup_bytes(key: &FontMarkupKey, markup: &(String, String)) -> usize {
    key.text.capacity()
        + key.font_family.capacity()
        + key.language.as_ref().map_or(0, String::capacity)
        + key.features.as_ref().map_or(0, String::capacity)
        + markup.0.capacity()
        + markup.1.capacity()
}

fn resolved_font_markup(text: &str, style: &CssStyle, x: f32) -> (String, String) {
    let runs = SvgRasterizeOps::html_text_paint_runs(text, style.raster_font_style());
    #[cfg(all(test, target_os = "macos"))]
    let run_families = runs.as_ref().map(|runs| {
        runs.iter()
            .map(|run| run.family.clone())
            .collect::<Vec<_>>()
    });
    let markup = font_runs_markup(text, style, runs, x);
    #[cfg(all(test, target_os = "macos"))]
    SvgRasterizeOps::record_actual_html_font_markup(
        style.used_font_family(),
        style.font_weight,
        run_families.as_deref(),
        &markup.0,
    );
    markup
}

fn font_runs_markup(
    text: &str,
    style: &CssStyle,
    runs: Option<Vec<HtmlPaintFontRun>>,
    x: f32,
) -> (String, String) {
    match runs {
        None => single_run_markup(text, style, None),
        Some(runs) if runs.len() <= 1 => single_run_markup(text, style, runs.first()),
        Some(runs) => multi_run_markup(style, runs, x),
    }
}

fn single_run_markup(
    text: &str,
    style: &CssStyle,
    run: Option<&HtmlPaintFontRun>,
) -> (String, String) {
    let family_markup = run.map_or_else(|| author_family_markup(style), resolved_run_markup);
    (family_markup, super::super::svg::escape_xml(text))
}

fn resolved_run_markup(run: &HtmlPaintFontRun) -> String {
    let family = resolved_family_markup(&run.family);
    if run.variations.is_empty() {
        return family;
    }
    let settings = run
        .variations
        .iter()
        .filter_map(|variation| {
            let tag = std::str::from_utf8(&variation.tag).ok()?;
            Some(format!("{} {}", CssStringLiteral(tag), variation.value))
        })
        .collect::<Vec<_>>()
        .join(", ");
    let settings = super::super::svg::escape_xml(&settings);
    format!("{family} font-optical-sizing=\"none\" font-variation-settings=\"{settings}\"")
}

fn resolved_family_markup(family: &str) -> String {
    let css_literal = CssStringLiteral(family);
    let serialized = format!("{css_literal}");
    format!(
        "font-family=\"{}\"",
        super::super::svg::escape_xml(&serialized)
    )
}

struct CssStringLiteral<'a>(&'a str);

impl Display for CssStringLiteral<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        cssparser::serialize_string(self.0, formatter)
    }
}

fn multi_run_markup(
    style: &CssStyle,
    runs: Vec<HtmlPaintFontRun>,
    origin_x: f32,
) -> (String, String) {
    /* WHY: 明示xでusvgのtext chunkを分け、別faceの欠落glyphがLatin runまで置換するのを防ぐ。 */
    let mut prefix = String::new();
    let content = runs
        .into_iter()
        .map(|run| {
            let x = if prefix.is_empty() {
                0.0
            } else {
                super::super::text_metrics::text_width(&prefix, style) + style.letter_spacing
            };
            let x = origin_x + x;
            prefix.push_str(&run.text);
            format!(
                "<tspan x=\"{x}\" {}>{}</tspan>",
                resolved_run_markup(&run),
                super::super::svg::escape_xml(&run.text)
            )
        })
        .collect::<String>();
    (author_family_markup(style), content)
}

fn author_family_markup(style: &CssStyle) -> String {
    format!(
        "font-family=\"{}\"",
        super::super::svg::escape_xml(style.used_font_family())
    )
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, FontMarkupCache, HtmlPaintFontRun};

    #[test]
    fn single_face_uses_resolved_family_and_escapes_text() {
        let mut style = CssStyle::browser_default();
        style.font_family = "Noto Sans, sans-serif".to_string();
        let mut cache = FontMarkupCache::default();
        let (family, content) = cache.font_markup("Summary & <text>", &style);
        assert_eq!(family, "font-family=\"&quot;Noto Sans&quot;\"");
        assert_eq!(content, "Summary &amp; &lt;text&gt;");
    }

    #[test]
    fn mixed_latin_and_japanese_emits_per_face_tspans() {
        let mut style = CssStyle::browser_default();
        style.font_family = "Noto Sans, sans-serif".to_string();
        let mut cache = FontMarkupCache::default();
        let (family, content) = cache.font_markup("A日本B", &style);
        assert!(family.contains("font-family="));
        assert!(content.contains("<tspan x=\"0\" font-family="));
        assert!(content.contains("A"));
        assert!(content.contains("日本"));
        assert!(content.contains("B"));
    }

    #[test]
    fn positioned_font_runs_follow_the_origin_and_typography_changes() {
        let mut style = CssStyle::browser_default();
        style.font_family = "Noto Sans, sans-serif".to_string();
        let mut cache = FontMarkupCache::default();
        let first = cache.font_markup_at("A日本B", &style, 10.0);
        assert!(first.1.contains("<tspan x=\"10\""));
        let shifted = cache.font_markup_at("A日本B", &style, 20.0);
        assert!(shifted.1.contains("<tspan x=\"20\""));
        assert_ne!(first, shifted);
        style.font_size *= 2.0;
        let enlarged = cache.font_markup_at("A日本B", &style, 10.0);
        assert_ne!(first, enlarged);
        style.letter_spacing = 1.0;
        assert_ne!(enlarged, cache.font_markup_at("A日本B", &style, 10.0));
    }

    #[test]
    fn font_markup_cache_reuses_entries_and_bounds_distinct_lines() {
        let style = CssStyle::browser_default();
        let mut cache = FontMarkupCache::default();
        let first = cache.font_markup("Repeated document line", &style);
        assert_eq!(cache.font_markup("Repeated document line", &style), first);
        assert_eq!(cache.entries.len(), 1);
        for index in 0..super::MAX_CACHED_FONT_MARKUPS {
            cache.font_markup(&format!("Document line {index}"), &style);
        }
        assert_eq!(cache.entries.len(), super::MAX_CACHED_FONT_MARKUPS);
        assert!(cache.retained_bytes <= super::MAX_CACHED_FONT_MARKUP_BYTES);
        assert_eq!(cache.font_markup("Repeated document line", &style), first);
        assert_eq!(cache.entries.len(), super::MAX_CACHED_FONT_MARKUPS);
    }

    #[test]
    fn empty_font_runs_preserve_author_family_and_xml_escaping() {
        let mut style = CssStyle::browser_default();
        style.font_family = "Example & <Family>".to_string();
        let (family, content) = super::single_run_markup("A & <B>", &style, None);
        assert_eq!(family, "font-family=\"Example &amp; &lt;Family&gt;\"");
        assert_eq!(content, "A &amp; &lt;B&gt;");
        assert_eq!(
            super::font_runs_markup("A & <B>", &style, None, 0.0),
            (family.clone(), content.clone())
        );
        assert_eq!(
            super::font_runs_markup("A & <B>", &style, Some(Vec::new()), 0.0),
            (family, content)
        );
        style.font_family = "system-ui, sans-serif".to_string();
        assert_eq!(
            super::font_runs_markup("text", &style, None, 0.0).0,
            "font-family=\"system-ui, sans-serif\""
        );
    }

    #[test]
    fn resolved_families_are_css_serialized_before_xml_escaping() -> Result<(), std::fmt::Error> {
        let style = CssStyle::browser_default();
        for family in [".Hidden Font", "Face \"quoted\" \\ path & <tag>"] {
            let expected_attribute = serialized_family_attribute(family)?;
            let single = super::font_runs_markup(
                "x",
                &style,
                Some(vec![HtmlPaintFontRun::plain(
                    family.to_string(),
                    "x".to_string(),
                )]),
                0.0,
            );
            assert_eq!(single.0, expected_attribute);

            let multi = super::font_runs_markup(
                "xy",
                &style,
                Some(vec![
                    HtmlPaintFontRun::plain(family.to_string(), "x".to_string()),
                    HtmlPaintFontRun::plain("Other Face".to_string(), "y".to_string()),
                ]),
                0.0,
            );
            assert!(multi.1.contains(&expected_attribute));
        }
        Ok(())
    }

    #[test]
    fn variable_font_markup_serializes_settings_escapes_xml_and_skips_invalid_tags() {
        let run = HtmlPaintFontRun {
            family: "Face & <tag>".to_string(),
            text: "x".to_string(),
            variations: vec![
                resvg::usvg::FontVariation::new(*b"wght", 650.0),
                resvg::usvg::FontVariation::new(*b"wdth", 80.0),
                resvg::usvg::FontVariation::new([0xff, 0xfe, 0xfd, 0xfc], 1.0),
            ],
        };

        let markup = super::resolved_run_markup(&run);

        assert_eq!(
            markup,
            "font-family=\"&quot;Face &amp; &lt;tag&gt;&quot;\" font-optical-sizing=\"none\" \
             font-variation-settings=\"&quot;wght&quot; 650, &quot;wdth&quot; 80\""
        );
    }

    fn serialized_family_attribute(family: &str) -> Result<String, std::fmt::Error> {
        let mut serialized = String::new();
        cssparser::serialize_string(family, &mut serialized)?;
        Ok(format!(
            "font-family=\"{}\"",
            super::super::super::svg::escape_xml(&serialized)
        ))
    }

    #[test]
    fn oversized_markup_is_not_retained_in_the_cache() {
        let mut cache = FontMarkupCache::default();
        let key = super::FontMarkupKey {
            text: "Document line".to_string(),
            font_family: "Example".to_string(),
            language: None,
            font_weight: CssStyle::browser_default().font_weight,
            italic: false,
            x: 0.0_f32.to_bits(),
            font_size: CssStyle::browser_default().font_size.to_bits(),
            letter_spacing: 0.0_f32.to_bits(),
            features: None,
        };
        cache.cache_markup(
            key,
            (
                String::new(),
                "x".repeat(super::MAX_CACHED_FONT_MARKUP_BYTES + 1),
            ),
        );
        assert_eq!(cache.retained_bytes, 0);
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn cached_payloads_evict_older_entries_before_exceeding_the_byte_budget() {
        let mut cache = FontMarkupCache::default();
        for text in ["First line", "Second line"] {
            cache.cache_markup(
                super::FontMarkupKey {
                    text: text.to_string(),
                    font_family: "Example".to_string(),
                    language: None,
                    font_weight: CssStyle::browser_default().font_weight,
                    italic: false,
                    x: 0.0_f32.to_bits(),
                    font_size: CssStyle::browser_default().font_size.to_bits(),
                    letter_spacing: 0.0_f32.to_bits(),
                    features: None,
                },
                (
                    String::new(),
                    "x".repeat(super::MAX_CACHED_FONT_MARKUP_BYTES / 2 + 1),
                ),
            );
        }
        assert_eq!(cache.entries.len(), 1);
        assert!(cache.retained_bytes <= super::MAX_CACHED_FONT_MARKUP_BYTES);
        assert!(cache.entries.keys().all(|key| key.text == "Second line"));
    }
}
