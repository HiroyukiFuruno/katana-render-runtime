use crate::svg_engine::usvg::{self, Node};
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};

pub(super) struct IsolatedMarkerMask;

impl IsolatedMarkerMask {
    pub(super) fn render(
        tree: &usvg::Tree,
        chain: &[usize],
        width: u32,
        height: u32,
    ) -> Result<Pixmap, String> {
        let mut isolated = tree.clone();
        isolate(&mut isolated.root, chain)?;
        isolated.clip_paths.clear();
        isolated.root.collect_clip_paths(&mut isolated.clip_paths);
        for clip in &isolated.clip_paths {
            if tree
                .clip_paths
                .iter()
                .filter(|original| Arc::ptr_eq(original, clip))
                .count()
                != 1
            {
                return Err("isolated clip registry differs from original".into());
            }
        }
        let mut output = Pixmap::new(width, height).ok_or("isolated pixmap unavailable")?;
        let scale = Transform::from_scale(
            width as f32 / tree.size().width(),
            height as f32 / tree.size().height(),
        );
        crate::svg_engine::resvg::render(&isolated, scale, &mut output.as_mut());
        Ok(output)
    }
}

fn isolate(group: &mut usvg::Group, chain: &[usize]) -> Result<(), String> {
    group.opacity = usvg::Opacity::ONE;
    group.blend_mode = usvg::BlendMode::Normal;
    if let Some((&index, rest)) = chain.split_first() {
        let node = group
            .children
            .get(index)
            .ok_or("isolated group index missing")?;
        let Node::Group(child) = node else {
            return Err("isolated path is not a group".into());
        };
        let mut child = child.clone();
        isolate(&mut child, rest)?;
        group.children = vec![Node::Group(child)];
    } else {
        group.children = vec![Node::Path(Box::new(white_path(group)?))];
    }
    group
        .calculate_bounding_boxes()
        .ok_or("isolated bounds unavailable")?;
    Ok(())
}

fn white_path(group: &usvg::Group) -> Result<usvg::Path, String> {
    let rect = white_bounds(group)?;
    let fill = usvg::Fill {
        paint: usvg::Paint::Color(usvg::Color::white()),
        ..usvg::Fill::default()
    };
    usvg::Path::new(
        String::new(),
        true,
        Some(fill),
        None,
        usvg::PaintOrder::default(),
        usvg::ShapeRendering::CrispEdges,
        Arc::new(tiny_skia::PathBuilder::from_rect(rect)),
        group.abs_transform,
    )
    .ok_or("white path unavailable".into())
}

fn white_bounds(group: &usvg::Group) -> Result<tiny_skia::Rect, String> {
    let clip = group
        .clip_path
        .as_ref()
        .ok_or("isolated target clip missing")?;
    let bounds = clip
        .root
        .stroke_bounding_box
        .transform(clip.transform)
        .ok_or("clip bounds unavailable")?;
    let padding = 1.0;
    tiny_skia::Rect::from_xywh(
        bounds.x() - padding,
        bounds.y() - padding,
        bounds.width() + 2.0 * padding,
        bounds.height() + 2.0 * padding,
    )
    .ok_or("white bounds invalid".into())
}
