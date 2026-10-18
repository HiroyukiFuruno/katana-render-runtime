use super::*;

#[path = "svg_rasterize_text_usvg_cmap_probe_weight_trace_face.rs"]
mod face;
#[path = "svg_rasterize_text_usvg_cmap_probe_weight_trace_selectors.rs"]
mod selectors;
#[path = "svg_rasterize_text_usvg_cmap_probe_weight_trace_spans.rs"]
mod spans;

pub(super) type Records = Arc<std::sync::Mutex<Vec<serde_json::Value>>>;

pub(super) fn run() -> Result<serde_json::Value, String> {
    let database = Arc::new(
        crate::markdown::svg_rasterize::text_shaping::html_font_db_for_text("system-ui", "日本"),
    );
    let mut options = resvg::usvg::Options {
        fontdb: Arc::clone(&database),
        ..resvg::usvg::Options::default()
    };
    let selected = Arc::new(std::sync::Mutex::new(Vec::new()));
    let fallback = Arc::new(std::sync::Mutex::new(Vec::new()));
    selectors::install(&mut options, Arc::clone(&selected), Arc::clone(&fallback));
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="48"><text id="weight-runs" x="0" y="32" font-family="system-ui" font-size="16"><tspan font-weight="400">日本</tspan><tspan font-weight="700">日本</tspan></text></svg>"#;
    spans::capture(svg, &options, selected, fallback)
}
