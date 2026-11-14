use super::HtmlDocumentNode;
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use sha2::{Digest, Sha256};

fn bytes(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value);
}

fn nodes(hash: &mut Sha256, values: &[HtmlDocumentNode]) {
    hash.update((values.len() as u64).to_le_bytes());
    for node in values {
        node_digest(hash, node);
    }
}

fn node_digest(hash: &mut Sha256, node: &HtmlDocumentNode) {
    match node {
        HtmlDocumentNode::Text(text) => {
            bytes(hash, b"text");
            bytes(hash, text.as_bytes());
        }
        HtmlDocumentNode::Element {
            node_id,
            tag,
            attributes,
            children,
        } => {
            bytes(hash, b"element");
            hash.update(node_id.to_le_bytes());
            bytes(hash, tag.as_bytes());
            hash.update((attributes.len() as u64).to_le_bytes());
            for (name, value) in attributes {
                bytes(hash, name.as_bytes());
                bytes(hash, value.as_bytes());
            }
            nodes(hash, children);
        }
    }
}

pub(super) fn projection(
    values: &[HtmlDocumentNode],
    session: &super::super::session::HtmlInteractiveSession,
    generation: u64,
) {
    if !SvgRasterizeOps::font_work_enabled() {
        return;
    }
    let mut digest = Sha256::new();
    nodes(&mut digest, values);
    SvgRasterizeOps::observe_font_work(
        "dom",
        serde_json::json!({
            "nodeSha256": digest.finalize().to_vec(), "generation": generation,
            "sourceSha256": Sha256::digest(session.source.raw_html.as_bytes()).to_vec(),
            "sourceBytes": session.source.raw_html.len(), "origin": session.source.origin.as_str(),
            "viewport": session.viewport, "scrollYBits": session.scroll_geometry.scroll_y.to_bits(),
        }),
    );
}
