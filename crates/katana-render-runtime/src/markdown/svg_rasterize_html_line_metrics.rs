use super::super::font::html_font_db_for_measured_text;
use super::super::measurement_diagnostic::{FaceMetricOutcome, FaceMetricPhase, FaceMetricScope};
use super::family_runs::resolved_font_runs;
use super::font::matching_font_face;
use resvg::usvg::fontdb::{Database, ID};
use std::{cell::RefCell, collections::HashMap};

/* WHY: 同一font条件のline-height計測を再利用し、本文文字列はcacheへ保持しない。 */
const MAX_LINE_METRIC_CACHE: usize = 256;

#[path = "svg_rasterize_html_line_metrics_persistent.rs"]
mod persistent;
#[path = "svg_rasterize_html_primary_baseline.rs"]
mod primary_baseline;
#[path = "svg_rasterize_html_line_metrics_uncached.rs"]
mod uncached;

thread_local! {
    static LINE_METRIC_CACHE: RefCell<HashMap<LineMetricKey, (f32, f32)>> =
        RefCell::new(HashMap::new());
}

pub(in super::super) fn clear_html_line_metrics_cache() {
    LINE_METRIC_CACHE.with(|cache| *cache.borrow_mut() = HashMap::new());
    primary_baseline::clear_primary_baseline_cache();
}

#[cfg(test)]
fn html_line_metrics_cache_len() -> usize {
    LINE_METRIC_CACHE.with(|cache| cache.borrow().len())
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct LineMetricKey {
    text_hash: u64,
    family_hash: u64,
    font_size: u32,
    font_weight: u16,
    italic: bool,
}

pub(in super::super) fn html_line_metrics(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
) -> Option<(f32, f32)> {
    let font_size = super::effective_html_font_size(font_size);
    let key = LineMetricKey::new(text, font_family, font_size, font_weight, italic);
    if let Some(metrics) = LINE_METRIC_CACHE.with(|cache| cache.borrow().get(&key).copied()) {
        super::super::measurement_diagnostic::line_finish(true, None);
        return Some(metrics);
    }
    let input = (text, font_family, font_size, font_weight, italic);
    let metrics = persistent::with_reuse(input, || {
        html_line_metrics_uncached(text, font_family, font_size, font_weight, italic)
    })?;
    LINE_METRIC_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= MAX_LINE_METRIC_CACHE
            && let Some(evicted_key) = cache.keys().next().copied()
        {
            cache.remove(&evicted_key);
        }
        cache.insert(key, metrics);
    });
    Some(metrics)
}

impl LineMetricKey {
    fn new(text: &str, family: &str, size: f32, weight: u16, italic: bool) -> Self {
        Self {
            text_hash: persistent::stable_hash(text),
            family_hash: persistent::stable_hash(family),
            font_size: size.to_bits(),
            font_weight: weight,
            italic,
        }
    }
}

fn html_line_metrics_uncached(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
) -> Option<(f32, f32)> {
    let started = super::super::measurement_diagnostic::line_start();
    let metrics = uncached::html_line_metrics_uncached_body(
        text,
        font_family,
        font_size,
        font_weight,
        italic,
        started.is_some(),
    );
    super::super::measurement_diagnostic::line_finish(false, started);
    metrics
}

fn line_metrics_for_face(
    database: &resvg::usvg::fontdb::Database,
    id: resvg::usvg::fontdb::ID,
    size: f32,
) -> Option<(f32, f32)> {
    match super::harfrust::cached_face_line_metrics(database, id, size) {
        Ok(Some(metrics)) => Some(metrics),
        Ok(None) => legacy_line_metrics(database, id, size),
        Err(()) => None,
    }
}

fn observed_line_metrics_for_face(
    database: &Database,
    id: ID,
    size: f32,
    detail_enabled: bool,
) -> Option<(f32, f32)> {
    if detail_enabled {
        diagnosed_line_metrics_for_face(database, id, size)
    } else {
        line_metrics_for_face(database, id, size)
    }
}

fn diagnosed_line_metrics_for_face(database: &Database, id: ID, size: f32) -> Option<(f32, f32)> {
    let detail = FaceMetricScope::new(true);
    match super::harfrust::diagnosed_cached_face_line_metrics(database, id, size) {
        Ok(Some(metrics)) => {
            detail.record_outcome(FaceMetricOutcome::Bridge);
            Some(metrics)
        }
        Ok(None) => {
            detail.record_outcome(FaceMetricOutcome::Legacy);
            detail.measure(FaceMetricPhase::Legacy, || {
                legacy_line_metrics(database, id, size)
            })
        }
        Err(()) => {
            detail.record_outcome(FaceMetricOutcome::Invalid);
            None
        }
    }
}

fn legacy_line_metrics(database: &Database, id: ID, size: f32) -> Option<(f32, f32)> {
    database.with_face_data(id, |data, index| face_line_metrics(data, index, size))?
}

fn face_line_metrics(data: &[u8], index: u32, size: f32) -> Option<(f32, f32)> {
    /* WHY: line metricsに不要なGSUB/GPOS shaping tablesを構築せず、同一faceのメトリクスだけ読む。 */
    let face = rustybuzz::ttf_parser::Face::parse(data, index).ok()?;
    Some(super::harfrust::parsed_face_line_metrics(&face, size))
}

#[cfg(test)]
fn html_text_baseline(
    family: &str,
    size: f32,
    weight: u16,
    italic: bool,
    line_height: f32,
) -> Option<f32> {
    html_text_baseline_for_text("", family, size, weight, italic, line_height, false)
}

/* WHY: normal は全 glyph run の line metrics、明示値は CSS の primary strut を使う。 */
pub(in super::super) fn html_text_baseline_for_text(
    text: &str,
    family: &str,
    size: f32,
    weight: u16,
    italic: bool,
    line_height: f32,
    normal_line_height: bool,
) -> Option<f32> {
    let size = super::effective_html_font_size(size);
    if normal_line_height {
        return html_line_metrics(text, family, size, weight, italic).map(|(ascent, _)| ascent);
    }
    primary_baseline::primary_baseline(family, size, weight, italic, line_height)
}

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_line_metrics_tests.rs"]
mod tests;
