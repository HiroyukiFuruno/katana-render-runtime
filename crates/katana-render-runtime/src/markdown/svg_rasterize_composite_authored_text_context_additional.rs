use resvg::usvg::{Text, roxmltree::Node};

const MIN_MULTILINE_CHUNKS: usize = 2;

pub(in crate::markdown::svg_rasterize) struct AdditionalTextProof {
    source_text: String,
    chunks: Vec<ChunkProof>,
}

struct ChunkProof {
    text: String,
    x: Option<f32>,
    y: Option<f32>,
    dy: f32,
}

impl AdditionalTextProof {
    pub(in crate::markdown::svg_rasterize) fn source_text(&self) -> &str {
        &self.source_text
    }

    pub(in crate::markdown::svg_rasterize) fn matches(&self, text: &Text) -> bool {
        if self.chunks.len() != text.chunks().len() {
            return false;
        }
        let mut offset = 0;
        self.chunks.iter().zip(text.chunks()).all(|(proof, chunk)| {
            let dy = match text.dy().get(offset) {
                Some(value) => *value,
                None => 0.0,
            };
            offset += chunk.text().chars().count();
            proof.text == chunk.text()
                && proof.x == chunk.x()
                && proof.y == chunk.y()
                && proof.dy == dy
        })
    }
}

pub(in crate::markdown::svg_rasterize) fn additional_text(
    root: Node<'_, '_>,
    styles: &[simplecss::Selector<'_>],
) -> Option<AdditionalTextProof> {
    if !super::svg_namespace(root) || super::in_ambiguous_context(root) || root_dy(root)? != 0.0 {
        return None;
    }
    let source_text = super::inventory::source_text(root)?;
    if source_text.trim().is_empty() {
        return None;
    }
    let elements = root
        .children()
        .filter(|node| node.is_element())
        .collect::<Vec<_>>();
    let chunks = multiline_chunks(root, &elements, styles)?;
    Some(AdditionalTextProof {
        source_text,
        chunks,
    })
}

fn multiline_chunks(
    root: Node<'_, '_>,
    elements: &[Node<'_, '_>],
    styles: &[simplecss::Selector<'_>],
) -> Option<Vec<ChunkProof>> {
    if elements.len() < MIN_MULTILINE_CHUNKS || root.children().any(meaningful_outer_text) {
        return None;
    }
    elements
        .iter()
        .enumerate()
        .map(|(index, child)| line_chunk(root, *child, index, styles))
        .collect()
}

fn line_chunk(
    root: Node<'_, '_>,
    child: Node<'_, '_>,
    index: usize,
    styles: &[simplecss::Selector<'_>],
) -> Option<ChunkProof> {
    if !simple_inherited_tspan(child, styles) {
        return None;
    }
    let x = optional_number(child, "x")??;
    let dy = number(child.attribute("dy")?)?;
    if (index == 0) != (dy == 0.0) {
        return None;
    }
    let y = if index == 0 {
        optional_number(root, "y")?
    } else {
        None
    };
    Some(ChunkProof {
        text: super::inventory::source_text(child)?,
        x: Some(x),
        y,
        dy,
    })
}

fn simple_inherited_tspan(node: Node<'_, '_>, styles: &[simplecss::Selector<'_>]) -> bool {
    node.has_tag_name("tspan")
        && super::svg_namespace(node)
        && !node.children().any(|child| child.is_element())
        && super::descendant_style_is_inherited(node, styles)
}

fn meaningful_outer_text(node: Node<'_, '_>) -> bool {
    node.is_text() && node.text().is_some_and(|text| !text.trim().is_empty())
}

fn root_dy(root: Node<'_, '_>) -> Option<f32> {
    match optional_number(root, "dy")? {
        Some(value) => Some(value),
        None => Some(0.0),
    }
}

fn optional_number(node: Node<'_, '_>, name: &str) -> Option<Option<f32>> {
    match node.attribute(name) {
        Some(value) => number(value).map(Some),
        None => Some(None),
    }
}

fn number(value: &str) -> Option<f32> {
    let value = value.trim().parse::<f32>().ok()?;
    value.is_finite().then_some(value)
}
