use super::PreparedSpan;
use resvg::usvg;
use tiny_skia::Point;

#[path = "svg_rasterize_html_glyph_fallback_anchors.rs"]
mod anchors;
#[path = "svg_rasterize_html_glyph_fallback_eligibility.rs"]
mod eligibility;
#[path = "svg_rasterize_html_glyph_fallback_layout_context.rs"]
mod layout_context;
#[path = "svg_rasterize_html_glyph_fallback_metrics.rs"]
mod metrics;
#[path = "svg_rasterize_html_glyph_fallback_positions.rs"]
mod positions;
#[path = "svg_rasterize_html_glyph_fallback_scripts.rs"]
mod scripts;
#[path = "svg_rasterize_html_glyph_fallback_source_spans.rs"]
mod source_spans;
#[path = "svg_rasterize_html_glyph_fallback_sources.rs"]
mod sources;

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_fallback_runs_tests.rs"]
mod tests;

use scripts::matching_script_fragments;
#[cfg(test)]
use scripts::{ScriptFragment, resolved_scripts};

pub(super) struct RunLayout {
    pub(super) css_scale: f32,
    pub(super) paint_css_size: f32,
    pub(super) css_positions: Vec<Point>,
}

pub(super) fn layout(
    database: &usvg::fontdb::Database,
    text: &usvg::Text,
    prepared: &[PreparedSpan],
) -> Result<Option<Vec<RunLayout>>, String> {
    #[cfg(test)]
    if std::env::var("KRR_HTML_FEATURE_DISABLE_FALLBACK_RUNS").as_deref() == Ok("1") {
        return Ok(None);
    }
    let Some(source) = eligibility::eligible_source(text, prepared) else {
        return Ok(None);
    };
    Ok(positions::layout(database, text, prepared, source))
}
