use super::super::super::reference_score_tests::{GIT_GRAPH_SOURCE, render_mermaid_svg};
use super::super::GeometryContract;
use resvg::usvg::roxmltree::Document;

#[test]
fn git_graph_rejects_invalid_paths_in_real_rendered_svg() -> Result<(), String> {
    let svg = render_mermaid_svg(GIT_GRAPH_SOURCE)?;
    GeometryContract::git_graph(&svg)?;
    let document = Document::parse(&svg).map_err(|error| error.to_string())?;
    let path = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node
                    .attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|class| class == "arrow"))
        })
        .ok_or("rendered git arrow missing")?;
    let attribute = path.attribute_node("d").ok_or("arrow path data missing")?;
    for invalid in ["M0 0", "M0 0 Lbroken"] {
        let mut changed = svg.clone();
        changed.replace_range(attribute.range_value(), invalid);
        assert!(GeometryContract::git_graph(&changed).is_err());
    }
    Ok(())
}
