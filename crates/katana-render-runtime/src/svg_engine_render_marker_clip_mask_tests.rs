use super::marker_isolated_mask_tests::IsolatedMarkerMask;
use crate::svg_engine::usvg::{self, Node};
use tiny_skia::{FillRule, Mask, PathBuilder, Pixmap, Transform};
use tiny_skia_path::PathSegment;

type ClipPlacement<'a> = (&'a usvg::ClipPath, Transform);
const RGBA_CHANNELS: usize = 4;
struct Context<'a> {
    tree: &'a usvg::Tree,
    width: u32,
    height: u32,
}

pub(super) struct MarkerClipMask;

impl MarkerClipMask {
    pub(super) fn verify(tree: &usvg::Tree, width: u32, height: u32) -> Result<usize, String> {
        if tree.root().transform() != Transform::identity() || tree.root().clip_path().is_some() {
            return Err("unsupported root transform or clip".into());
        }
        let scale = Transform::from_scale(
            width as f32 / tree.size().width(),
            height as f32 / tree.size().height(),
        );
        let mut stack = Vec::new();
        let mut chain = Vec::new();
        let context = Context {
            tree,
            width,
            height,
        };
        let count = walk(&context, tree.root(), scale, &mut stack, &mut chain)?;
        if count == 0 {
            return Err("no marker clips verified".into());
        }
        Ok(count)
    }

    pub(super) fn is_rectangle(path: &tiny_skia_path::Path) -> bool {
        let segments: Vec<_> = path.segments().collect();
        match segments.as_slice() {
            [
                PathSegment::MoveTo(a),
                PathSegment::LineTo(b),
                PathSegment::LineTo(c),
                PathSegment::LineTo(d),
                PathSegment::Close,
            ] => a.y == b.y && b.x == c.x && c.y == d.y && d.x == a.x && a.x != b.x && a.y != d.y,
            _ => false,
        }
    }
}

fn walk<'a>(
    context: &Context<'a>,
    group: &'a usvg::Group,
    parent: Transform,
    stack: &mut Vec<ClipPlacement<'a>>,
    chain: &mut Vec<usize>,
) -> Result<usize, String> {
    if group.mask.is_some() || !group.filters.is_empty() {
        return Err("unsupported group effects".into());
    }
    let transform = parent.pre_concat(group.transform());
    let has_clip = if let Some(current) = group.clip_path() {
        stack.push((current, transform));
        verify_placement(context, stack, chain)?;
        true
    } else {
        false
    };
    let mut count = usize::from(has_clip);
    count += walk_children(context, group, transform, stack, chain)?;
    if has_clip {
        stack.pop();
    }
    Ok(count)
}

fn walk_children<'a>(
    context: &Context<'a>,
    group: &'a usvg::Group,
    transform: Transform,
    stack: &mut Vec<ClipPlacement<'a>>,
    chain: &mut Vec<usize>,
) -> Result<usize, String> {
    let mut count = 0;
    for (index, node) in group.children().iter().enumerate() {
        if let Node::Group(child) = node {
            chain.push(index);
            count += walk(context, child, transform, stack, chain)?;
            chain.pop();
        }
    }
    Ok(count)
}

fn verify_placement(
    context: &Context<'_>,
    stack: &[ClipPlacement<'_>],
    chain: &[usize],
) -> Result<(), String> {
    let expected = verify_stack(stack, context.width, context.height)?;
    let rendered = IsolatedMarkerMask::render(context.tree, chain, context.width, context.height)?;
    verify_pixels(&rendered, &expected)
}

fn verify_stack(stack: &[ClipPlacement<'_>], width: u32, height: u32) -> Result<Vec<bool>, String> {
    let pixel_count =
        usize::try_from(u64::from(width) * u64::from(height)).map_err(|error| error.to_string())?;
    let mut expected = vec![true; pixel_count];
    for (current, transform) in stack {
        let geometry = marker_rect(current)?;
        let mut mask = Mask::new(width, height).ok_or("expected mask unavailable")?;
        mask.fill_path(
            &geometry,
            FillRule::Winding,
            false,
            transform.pre_concat(current.transform()),
        );
        for (value, coverage) in expected.iter_mut().zip(mask.data()) {
            if *coverage != 0 && *coverage != u8::MAX {
                return Err("expected mask is not binary".into());
            }
            *value &= *coverage == u8::MAX;
        }
    }
    Ok(expected)
}

fn verify_pixels(actual: &Pixmap, expected: &[bool]) -> Result<(), String> {
    /* WHY: 画面外や親clipとの交差が空の場合も、期待maskとの全画素一致で判定する。 */
    for (pixel, covered) in actual.pixels().iter().zip(expected) {
        let value = if *covered { u8::MAX } else { 0 };
        if [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()] != [value; RGBA_CHANNELS] {
            return Err("marker mask differs from independent geometry".into());
        }
    }
    Ok(())
}

fn marker_rect(clip: &usvg::ClipPath) -> Result<tiny_skia_path::Path, String> {
    let [Node::Path(path)] = clip.root.children.as_slice() else {
        return Err("unknown clip structure".into());
    };
    if clip.clip_path.is_some()
        || clip.root.clip_path.is_some()
        || clip.root.mask.is_some()
        || !clip.root.filters.is_empty()
        || path.rendering_mode != usvg::ShapeRendering::CrispEdges
        || !MarkerClipMask::is_rectangle(&path.data)
    {
        return Err("unknown marker clip geometry".into());
    }
    Ok(PathBuilder::from_rect(path.data.bounds()))
}
