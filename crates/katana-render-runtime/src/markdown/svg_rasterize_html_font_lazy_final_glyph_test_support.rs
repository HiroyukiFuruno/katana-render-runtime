use crate::markdown::svg_rasterize::SvgRasterizeOps;

use super::MAX_FINAL_GLYPH_RECORDS;

pub(super) const PRIMARY_TEST_FONT_SIZE: usize = 16;
pub(super) const ALTERNATE_TEST_FONT_SIZE: usize = 17;

pub(super) fn font_tree(
    database: resvg::usvg::fontdb::Database,
) -> Result<resvg::usvg::Tree, String> {
    let family = database
        .faces()
        .find_map(|face| face.families.first().map(|family| family.0.clone()))
        .ok_or("fixture database has no font family")?;
    let options = resvg::usvg::Options {
        font_family: family,
        fontdb: std::sync::Arc::new(database),
        ..Default::default()
    };
    resvg::usvg::Tree::from_str(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"><text id="audit-run" x="1" y="24" font-size="16">AV</text></svg>"#,
        &options,
    )
    .map_err(|error| error.to_string())
}

pub(super) fn text_span(
    tree: &resvg::usvg::Tree,
) -> Result<(&resvg::usvg::Text, &resvg::usvg::layout::Span), String> {
    let Some(resvg::usvg::Node::Text(text)) = tree.node_by_id("audit-run") else {
        return Err("font audit fixture text is missing".to_owned());
    };
    let span = text
        .layouted()
        .first()
        .ok_or("font audit fixture span is missing")?;
    Ok((text, span))
}

pub(super) fn svg_with_many_text_spans() -> String {
    let spans = (0..=MAX_FINAL_GLYPH_RECORDS)
        .map(|index| {
            let size = if index % 2 == 0 {
                PRIMARY_TEST_FONT_SIZE
            } else {
                ALTERNATE_TEST_FONT_SIZE
            };
            format!(r#"<tspan font-size="{size}">A</tspan>"#)
        })
        .collect::<String>();
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><text x="0" y="20">{spans}</text></svg>"#
    )
}

pub(super) fn svg_with_repeated_glyph_text() -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="24"><text x="1" y="20"><tspan font-size="{PRIMARY_TEST_FONT_SIZE}">A</tspan><tspan font-size="{ALTERNATE_TEST_FONT_SIZE}">A</tspan></text></svg>"#
    )
}

pub(super) fn audit_svg(svg: &str) -> Result<serde_json::Value, String> {
    let (render, audit) = SvgRasterizeOps::with_actual_font_source_audit(|| {
        SvgRasterizeOps::rasterize_html_svg(svg, 1.0)
    })
    .map_err(|error| error.to_string())?;
    render.map_err(|error| error.to_string())?;
    Ok(audit)
}

pub(super) fn root_tree(audit: &serde_json::Value) -> Result<&serde_json::Value, String> {
    audit["finalGlyphTrees"]
        .as_array()
        .and_then(|trees| trees.iter().find(|tree| tree["treePath"] == "root"))
        .ok_or_else(|| "root glyph tree evidence is absent".to_owned())
}
