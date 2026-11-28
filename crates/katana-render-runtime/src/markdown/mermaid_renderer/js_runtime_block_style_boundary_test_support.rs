use super::{BLOCK, layout};
use resvg::usvg::roxmltree::{Document, Node};

pub(super) fn parse(svg: &str) -> Result<Document<'_>, String> {
    Document::parse(svg).map_err(|error| error.to_string())
}

pub(super) fn label_rect<'a, 'input>(
    document: &'a Document<'input>,
) -> Result<Node<'a, 'input>, String> {
    let group = layout::semantic_node(document.root_element(), "-ID")?;
    layout::label_rectangle(group)
}

pub(super) fn with_parent_style(style: &str) -> String {
    BLOCK.replacen(
        r#"class="label-container""#,
        &format!(r#"class="label-container" style="{style}""#),
        1,
    )
}

pub(super) fn with_stylesheet(style: &str) -> String {
    BLOCK.replacen(
        r#"<g class="node""#,
        &format!(r#"<style>{style}</style><g class="node""#),
        1,
    )
}
