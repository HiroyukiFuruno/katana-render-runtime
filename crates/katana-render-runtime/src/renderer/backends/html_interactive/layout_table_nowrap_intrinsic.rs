use super::{CssStyle, CssWhiteSpace, HtmlDocumentNode, TableIntrinsic, inline_metrics};

pub(super) fn min_content_width(children: &[HtmlDocumentNode], inherited: &CssStyle) -> f32 {
    let mut lines = vec![0.0];
    for child in children {
        append_width(
            child,
            &inline_metrics::child_style(child, inherited),
            &mut lines,
        );
    }
    lines.into_iter().fold(0.0, f32::max)
}

fn append_width(node: &HtmlDocumentNode, style: &CssStyle, lines: &mut Vec<f32>) {
    match node {
        HtmlDocumentNode::Text(text) => append_text(text, style, lines),
        HtmlDocumentNode::Element { tag, .. } if tag == "br" => lines.push(0.0),
        HtmlDocumentNode::Element { .. }
            if style.white_space != CssWhiteSpace::NoWrap || style.inline_atomic =>
        {
            append_element(node, style, lines);
        }
        HtmlDocumentNode::Element { children, .. } => {
            let first_line = lines.len() - 1;
            for child in children {
                append_width(child, &inline_metrics::child_style(child, style), lines);
            }
            let outer = style.intrinsic_outer_width(0.0);
            for line in &mut lines[first_line..] {
                *line += outer;
            }
        }
    }
}

fn append_text(text: &str, style: &CssStyle, lines: &mut [f32]) {
    if let Some(line) = lines.last_mut() {
        *line += inline_metrics::table_no_wrap_text_width(text, style);
    }
}

fn append_element(node: &HtmlDocumentNode, style: &CssStyle, lines: &mut [f32]) {
    if let Some(line) = lines.last_mut() {
        *line += TableIntrinsic::min_content_width(node, style);
    }
}
