use super::{Records, face};

pub(super) fn capture(
    svg: &str,
    options: &resvg::usvg::Options<'static>,
    selected_records: Records,
    fallback_records: Records,
) -> Result<serde_json::Value, String> {
    let tree = crate::markdown::svg_rasterize::text_shaping::with_validated_tree_parse(|| {
        resvg::usvg::Tree::from_str(svg, options)
    })
    .map_err(|error| error.to_string())?;
    let text = match tree.node_by_id("weight-runs") {
        Some(resvg::usvg::Node::Text(text)) => text,
        _ => return Err("mixed-weight SVG text node missing".into()),
    };
    let spans = text
        .chunks()
        .iter()
        .flat_map(|chunk| chunk.spans())
        .zip(text.layouted())
        .map(|(requested, span)| face::span_record(requested, span, tree.fontdb()))
        .collect::<Result<Vec<_>, _>>()?;
    let selected = captured(&selected_records)?;
    let fallback = captured(&fallback_records)?;
    Ok(
        serde_json::json!({"spans": spans, "initialFontSelections": selected,
        "fallbackSelections": fallback}),
    )
}

fn captured(records: &Records) -> Result<Vec<serde_json::Value>, String> {
    records
        .lock()
        .map(|records| records.clone())
        .map_err(|_| "font trace lock poisoned".into())
}
