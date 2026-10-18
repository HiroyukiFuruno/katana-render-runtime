use super::super::types::LayoutResult;
use super::{SvgMeasurementStats, TextMeasurementDiagnosticStats};

#[path = "text_measurement_face_metric_detail.rs"]
mod face_metric_detail;
use face_metric_detail::{FACE_DETAIL_COUNT, face_detail_metrics};

const METRICS_PER_GROUP: usize = 8;
const BASE_METRIC_COUNT: usize = 16;
const LINE_METRIC_DETAIL_COUNT: usize = 7;
const LAYOUT_SVG_METRIC_COUNT: usize =
    BASE_METRIC_COUNT + METRICS_PER_GROUP + LINE_METRIC_DETAIL_COUNT + FACE_DETAIL_COUNT;
const NANOS_PER_MICROSECOND: usize = 1_000;

pub(super) fn layout_svg_metrics(
    rendered: &LayoutResult,
    flow_before: (usize, usize),
    flow_after: (usize, usize),
    text_stats: TextMeasurementDiagnosticStats,
    svg_stats: SvgMeasurementStats,
) -> [(&'static str, usize); LAYOUT_SVG_METRIC_COUNT] {
    let base = base_layout_svg_metrics(rendered, flow_before, flow_after, text_stats);
    let svg = svg_measurement_metrics(svg_stats);
    let line_detail = line_metric_detail_metrics(svg_stats);
    let face_detail = face_detail_metrics(svg_stats);
    std::array::from_fn(|index| {
        if index < BASE_METRIC_COUNT {
            base[index]
        } else if index < BASE_METRIC_COUNT + METRICS_PER_GROUP {
            svg[index - BASE_METRIC_COUNT]
        } else if index < BASE_METRIC_COUNT + METRICS_PER_GROUP + LINE_METRIC_DETAIL_COUNT {
            line_detail[index - BASE_METRIC_COUNT - METRICS_PER_GROUP]
        } else {
            face_detail[index - BASE_METRIC_COUNT - METRICS_PER_GROUP - LINE_METRIC_DETAIL_COUNT]
        }
    })
}

fn base_layout_svg_metrics(
    rendered: &LayoutResult,
    flow_before: (usize, usize),
    flow_after: (usize, usize),
    text_stats: TextMeasurementDiagnosticStats,
) -> [(&'static str, usize); BASE_METRIC_COUNT] {
    let legacy = legacy_layout_svg_metrics(rendered, flow_before, flow_after, text_stats);
    let timing = timing_metrics(text_stats);
    std::array::from_fn(|index| {
        if index < METRICS_PER_GROUP {
            legacy[index]
        } else {
            timing[index - METRICS_PER_GROUP]
        }
    })
}

fn legacy_layout_svg_metrics(
    rendered: &LayoutResult,
    flow_before: (usize, usize),
    flow_after: (usize, usize),
    text_stats: TextMeasurementDiagnosticStats,
) -> [(&'static str, usize); METRICS_PER_GROUP] {
    [
        ("svg_bytes", rendered.svg.len()),
        ("text_width_lookups", text_stats.lookups),
        ("text_width_hits", text_stats.hits),
        ("text_width_misses", text_stats.misses),
        ("text_width_measured_calls", text_stats.measured_calls),
        ("text_width_measured_us", micros(text_stats.measured_ns)),
        (
            "flow_cache_hits",
            flow_after.0.saturating_sub(flow_before.0),
        ),
        (
            "flow_cache_misses",
            flow_after.1.saturating_sub(flow_before.1),
        ),
    ]
}

fn timing_metrics(
    text_stats: TextMeasurementDiagnosticStats,
) -> [(&'static str, usize); METRICS_PER_GROUP] {
    [
        ("css_style_calls", text_stats.style_calls),
        ("css_style_us", micros(text_stats.style_ns)),
        ("taffy_layout_calls", text_stats.taffy_calls),
        ("taffy_layout_us", micros(text_stats.taffy_ns)),
        ("text_total_calls", text_stats.text_total_calls),
        ("text_total_us", micros(text_stats.text_total_ns)),
        ("wrap_calls", text_stats.wrap_calls),
        ("wrap_us", micros(text_stats.wrap_ns)),
    ]
}

fn svg_measurement_metrics(
    svg_stats: SvgMeasurementStats,
) -> [(&'static str, usize); METRICS_PER_GROUP] {
    [
        ("line_metric_calls", svg_stats.line_calls),
        ("line_metric_hits", svg_stats.line_hits),
        ("line_metric_misses", svg_stats.line_misses),
        ("line_metric_uncached_us", svg_stats.line_uncached_us),
        ("primary_baseline_calls", svg_stats.baseline_calls),
        ("primary_baseline_hits", svg_stats.baseline_hits),
        ("primary_baseline_misses", svg_stats.baseline_misses),
        (
            "primary_baseline_uncached_us",
            svg_stats.baseline_uncached_us,
        ),
    ]
}

fn line_metric_detail_metrics(
    svg_stats: SvgMeasurementStats,
) -> [(&'static str, usize); LINE_METRIC_DETAIL_COUNT] {
    [
        ("line_metric_db_us", micros(svg_stats.line_metric_db_ns)),
        (
            "line_metric_matching_us",
            micros(svg_stats.line_metric_matching_ns),
        ),
        ("line_metric_runs_us", micros(svg_stats.line_metric_runs_ns)),
        (
            "line_metric_faces_us",
            micros(svg_stats.line_metric_faces_ns),
        ),
        (
            "line_metric_face_file_calls",
            svg_stats.line_metric_face_file_calls,
        ),
        (
            "line_metric_face_binary_calls",
            svg_stats.line_metric_face_binary_calls,
        ),
        (
            "line_metric_face_shared_calls",
            svg_stats.line_metric_face_shared_calls,
        ),
    ]
}

fn micros(nanoseconds: usize) -> usize {
    nanoseconds / NANOS_PER_MICROSECOND
}

#[cfg(test)]
#[path = "text_measurement_diagnostic_metrics_tests.rs"]
mod tests;
