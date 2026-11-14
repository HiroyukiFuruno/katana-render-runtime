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

#[test]
fn git_graph_rejects_missing_commit_identity_in_rendered_svg() -> Result<(), String> {
    let svg = render_mermaid_svg(GIT_GRAPH_SOURCE)?;
    GeometryContract::git_graph(&svg)?;
    let document = Document::parse(&svg).map_err(|error| error.to_string())?;
    let circle = document
        .descendants()
        .find(|node| {
            node.has_tag_name("circle")
                && node
                    .attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|class| class == "base"))
        })
        .ok_or("base commit missing")?;
    let class = circle
        .attribute_node("class")
        .ok_or("commit class missing")?;
    let changed_class = class.value().replace("base", "unknown");
    let mut changed = svg.clone();
    changed.replace_range(class.range_value(), &changed_class);
    assert_eq!(
        GeometryContract::git_graph(&changed).err().as_deref(),
        Some("semantic element must be unique")
    );
    Ok(())
}
