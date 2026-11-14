use crate::markdown::svg_rasterize::SvgRasterizeOps;
use resvg::usvg;

#[test]
fn inline_style_leading_empty_declaration_selects_bold_in_krr_parse() -> Result<(), String> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="40"><text id="run" y="-10.1" font-family="Trebuchet MS" font-size="16" style="; font-weight: bold;"><tspan x="0" y="-.1em" dy="1.1em"><tspan font-style="normal" font-weight="normal" style="; font-weight: bold;">test_req</tspan></tspan></text></svg>"#;
    let compatible = SvgRasterizeOps::preprocess_for_rasterizer(svg);
    let tree = super::super::super::parse(&compatible, svg).map_err(|error| error.to_string())?;
    assert_inline_bold_selection(&tree)
}

fn assert_inline_bold_selection(tree: &usvg::Tree) -> Result<(), String> {
    let Some(usvg::Node::Text(text)) = tree.node_by_id("run") else {
        return Err("inline-style text missing".to_owned());
    };
    let spans = text
        .chunks()
        .iter()
        .flat_map(|chunk| chunk.spans())
        .collect::<Vec<_>>();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].font().weight(), 700);
    let glyph = text
        .layouted()
        .iter()
        .flat_map(|span| span.positioned_glyphs.iter())
        .next()
        .ok_or("inline-style glyph missing")?;
    let face = tree
        .fontdb()
        .face(glyph.font)
        .ok_or("inline-style selected face missing")?;
    assert_eq!(face.weight, usvg::fontdb::Weight(700));
    Ok(())
}
