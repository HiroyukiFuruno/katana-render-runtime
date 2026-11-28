use super::super::layout::DeferredPaint;
use super::super::types::ElementBox;
use std::collections::HashMap;

pub(in crate::renderer::backends::html_interactive) struct PaintClip {
    pub(in crate::renderer::backends::html_interactive) id: u64,
    pub(in crate::renderer::backends::html_interactive) depth: usize,
}

pub(in crate::renderer::backends::html_interactive) enum PaintFragment {
    Svg(String),
    Context {
        background: String,
        content: String,
        children: Vec<DeferredPaint>,
        rotation: Option<String>,
        opacity: f32,
    },
}

pub(in crate::renderer::backends::html_interactive) fn apply_deferred_clip(
    paint: &mut DeferredPaint,
    owner: u64,
    id: u64,
    depth: usize,
    indices: &HashMap<u64, usize>,
    boxes: &[ElementBox],
) {
    let eligible = indices
        .get(&paint.node_id)
        .and_then(|index| boxes.get(*index))
        .is_some_and(|item| {
            item.overflow_clips
                .iter()
                .any(|clip| clip.owner_node_id == owner)
        });
    if eligible {
        paint.clips.push(PaintClip { id, depth });
    }
    if let PaintFragment::Context { children, .. } = &mut paint.fragment {
        for child in children {
            apply_deferred_clip(child, owner, id, depth, indices, boxes);
        }
    }
}

pub(super) fn serialize_paint(paint: DeferredPaint, inherited: &[Option<String>]) -> String {
    match paint.fragment {
        PaintFragment::Svg(svg) => apply_paint_operations(svg, &paint.clips, inherited),
        PaintFragment::Context {
            background,
            content,
            children,
            rotation,
            opacity,
        } => {
            let mut rotations = inherited.to_vec();
            rotations.push(rotation);
            let mut svg = String::new();
            if opacity < 1.0 {
                svg.push_str(&format!(r#"<g opacity="{opacity}">"#));
            }
            svg.push_str(&apply_paint_operations(
                background,
                &paint.clips,
                &rotations,
            ));
            serialize_context_content(&mut svg, content, children, &paint.clips, &rotations);
            if opacity < 1.0 {
                svg.push_str("</g>");
            }
            svg
        }
    }
}

fn serialize_context_content(
    svg: &mut String,
    content: String,
    children: Vec<DeferredPaint>,
    clips: &[PaintClip],
    rotations: &[Option<String>],
) {
    let mut foreground = Vec::new();
    for child in children {
        if child.z_index < 0 {
            svg.push_str(&serialize_paint(child, rotations));
        } else {
            foreground.push(child);
        }
    }
    svg.push_str(&apply_paint_operations(content, clips, rotations));
    for child in foreground {
        svg.push_str(&serialize_paint(child, rotations));
    }
}

fn apply_paint_operations(
    svg: String,
    clips: &[PaintClip],
    rotations: &[Option<String>],
) -> String {
    if svg.is_empty() || (clips.is_empty() && rotations.iter().all(Option::is_none)) {
        return svg;
    }
    let mut prefix = String::new();
    let mut count = 0;
    for depth in 0..=rotations.len() {
        for clip in clips.iter().filter(|clip| clip.depth == depth) {
            prefix.push_str(&format!(r#"<g clip-path="url(#krr-clip-{})">"#, clip.id));
            count += 1;
        }
        if let Some(Some(rotation)) = rotations.get(depth) {
            prefix.push_str(rotation);
            count += 1;
        }
    }
    prefix.push_str(&svg);
    for _ in 0..count {
        prefix.push_str("</g>");
    }
    prefix
}
