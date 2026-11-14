use super::*;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_text_usvg_cmap_probe_weight_trace.rs"]
mod trace;

#[cfg(target_os = "macos")]
#[test]
fn html_system_ui_mixed_japanese_spans_trace_actual_usvg_faces() -> Result<(), String> {
    let trace = trace::run()?;
    let spans = trace["spans"]
        .as_array()
        .ok_or("mixed-weight SVG span trace missing")?;
    assert!(
        spans.len() >= 2,
        "expected normal and bold spans: {spans:?}"
    );
    assert!(
        spans
            .iter()
            .all(|span| { span["glyph_count"].as_u64().is_some_and(|count| count > 0) })
    );
    assert!(trace["initialFontSelections"].as_array().is_some());
    assert!(trace["fallbackSelections"].as_array().is_some());
    println!("KRR_SYSTEM_UI_WEIGHT_TRACE={trace}");
    Ok(())
}
