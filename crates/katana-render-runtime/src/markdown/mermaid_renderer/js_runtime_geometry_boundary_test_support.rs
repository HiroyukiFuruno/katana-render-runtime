use resvg::usvg::roxmltree::{Document, Node};

pub(super) fn element(svg: &str) -> Result<Document<'_>, String> {
    Document::parse(svg).map_err(|error| error.to_string())
}

pub(super) fn first_element<'a>(
    document: &'a Document<'a>,
    tag: &str,
) -> Result<Node<'a, 'a>, String> {
    document
        .descendants()
        .find(|node| node.has_tag_name(tag))
        .ok_or_else(|| format!("{tag} fixture missing"))
}
