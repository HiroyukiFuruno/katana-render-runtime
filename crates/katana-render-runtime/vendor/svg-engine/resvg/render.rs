// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::svg_engine::resvg::OptionLog;

pub struct Context {
    pub max_bbox: tiny_skia::IntRect,
}

pub fn render_nodes(
    parent: &crate::svg_engine::usvg::Group,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    for node in parent.children() {
        render_node(node, ctx, transform, pixmap);
    }
}

pub fn render_node(
    node: &crate::svg_engine::usvg::Node,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    match node {
        crate::svg_engine::usvg::Node::Group(group) => {
            render_group(group, ctx, transform, pixmap);
        }
        crate::svg_engine::usvg::Node::Path(path) => {
            crate::svg_engine::resvg::path::render(
                path,
                tiny_skia::BlendMode::SourceOver,
                ctx,
                transform,
                pixmap,
            );
        }
        crate::svg_engine::usvg::Node::Image(image) => {
            crate::svg_engine::resvg::image::render(image, transform, pixmap);
        }
        crate::svg_engine::usvg::Node::Text(text) => {
            render_group(text.flattened(), ctx, transform, pixmap);
        }
    }
}

fn render_group(
    group: &crate::svg_engine::usvg::Group,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
    let transform = transform.pre_concat(group.transform());

    if !group.should_isolate() {
        render_nodes(group, ctx, transform, pixmap);
        return Some(());
    }

    let rest = layer_transform(group, transform);
    let inv_rest = rest.invert()?;

    let max_bbox = ctx.max_bbox.to_rect().transform(inv_rest)?.round_out()?;
    let (layer, ibbox) =
        render_layer(group, &Context { max_bbox }, inv_rest.pre_concat(transform))?;
    composite_layer(group, &layer, ibbox, rest, pixmap);
    Some(())
}

fn composite_layer(
    group: &crate::svg_engine::usvg::Group,
    layer: &tiny_skia::Pixmap,
    ibbox: tiny_skia::IntRect,
    rest: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    let paint = tiny_skia::Paint {
        shader: tiny_skia::Pattern::new(
            layer.as_ref(),
            tiny_skia::SpreadMode::Pad,
            tiny_skia::FilterQuality::Bilinear,
            group.opacity().get(),
            tiny_skia::Transform::from_translate(ibbox.x() as f32, ibbox.y() as f32),
        ),
        blend_mode: convert_blend_mode(group.blend_mode()),
        anti_alias: true,
        ..tiny_skia::Paint::default()
    };
    let path = tiny_skia::PathBuilder::from_rect(ibbox.to_rect());
    pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, rest, None);

}

fn layer_transform(group: &crate::svg_engine::usvg::Group, transform: tiny_skia::Transform) -> tiny_skia::Transform {
    if group.filters().is_empty() {
        tiny_skia::Transform::identity()
    } else {
        let sx = transform.sx.hypot(transform.ky);
        let sy = transform.kx.hypot(transform.sy);
        tiny_skia::Transform::from_row(
            transform.sx / sx,
            transform.ky / sx,
            transform.kx / sy,
            transform.sy / sy,
            0.0,
            0.0,
        )
    }
}

/// Renders an isolated group into a new layer, returning it and its position.
fn render_layer(
    group: &crate::svg_engine::usvg::Group,
    ctx: &Context,
    transform: tiny_skia::Transform,
) -> Option<(tiny_skia::Pixmap, tiny_skia::IntRect)> {
    let bbox = group.layer_bounding_box().transform(transform)?;
    let ibbox = layer_bbox(group, ctx, bbox)?;

    let shift_ts = {
        // Original shift.
        let mut dx = bbox.x();
        let mut dy = bbox.y();

        // Account for subpixel positioned layers.
        dx -= bbox.x() - ibbox.x() as f32;
        dy -= bbox.y() - ibbox.y() as f32;

        tiny_skia::Transform::from_translate(-dx, -dy)
    };

    let transform = shift_ts.pre_concat(transform);

    let mut sub_pixmap = tiny_skia::Pixmap::new(ibbox.width(), ibbox.height())
        .log_none(|| log::warn!("Failed to allocate a group layer for: {:?}.", ibbox))?;

    render_nodes(group, ctx, transform, &mut sub_pixmap.as_mut());
    apply_layer_effects(group, ctx, transform, &mut sub_pixmap);
    Some((sub_pixmap, ibbox))
}

fn layer_bbox(
    group: &crate::svg_engine::usvg::Group,
    ctx: &Context,
    bbox: tiny_skia::NonZeroRect,
) -> Option<tiny_skia::IntRect> {
    let mut ibbox = if group.filters().is_empty() {
        // Convert group bbox into an integer one, expanding each side outwards by 2px
        // to make sure that anti-aliased pixels would not be clipped.
        tiny_skia::IntRect::from_xywh(
            (bbox.x().floor() as i32).checked_sub(2)?,
            (bbox.y().floor() as i32).checked_sub(2)?,
            (bbox.width().ceil() as u32).checked_add(4)?,
            (bbox.height().ceil() as u32).checked_add(4)?,
        )?
    } else {
        filtered_layer_bbox(bbox, ctx)?
    };

    // Make sure our layer is not bigger than 4x the canvas size.
    // This is required to prevent huge layers.
    if group.filters().is_empty() {
        ibbox = crate::svg_engine::resvg::geom::fit_to_rect(ibbox, ctx.max_bbox)?;
    }

    Some(ibbox)
}

fn filtered_layer_bbox(bbox: tiny_skia::NonZeroRect, ctx: &Context) -> Option<tiny_skia::IntRect> {
    // The filter region already acts as a clipping region and must not be expanded.
    let bbox = tiny_skia::IntRect::from_xywh(
        bbox.x().floor() as i32,
        bbox.y().floor() as i32,
        bbox.width().ceil().max(1.0) as u32,
        bbox.height().ceil().max(1.0) as u32,
    )?;
    crate::svg_engine::resvg::geom::fit_to_rect(bbox, ctx.max_bbox)
}

fn apply_layer_effects(
    group: &crate::svg_engine::usvg::Group,
    ctx: &Context,
    transform: tiny_skia::Transform,
    sub_pixmap: &mut tiny_skia::Pixmap,
) {
    if !group.filters().is_empty() {
        for filter in group.filters() {
            crate::svg_engine::resvg::filter::apply(filter, transform, sub_pixmap);
        }
    }

    if let Some(clip_path) = group.clip_path() {
        crate::svg_engine::resvg::clip::apply(clip_path, transform, sub_pixmap);
    }

    if let Some(mask) = group.mask() {
        crate::svg_engine::resvg::mask::apply(mask, ctx, transform, sub_pixmap);
    }

}

pub fn convert_blend_mode(mode: crate::svg_engine::usvg::BlendMode) -> tiny_skia::BlendMode {
    match mode {
        crate::svg_engine::usvg::BlendMode::Normal => tiny_skia::BlendMode::SourceOver,
        crate::svg_engine::usvg::BlendMode::Multiply => tiny_skia::BlendMode::Multiply,
        crate::svg_engine::usvg::BlendMode::Screen => tiny_skia::BlendMode::Screen,
        crate::svg_engine::usvg::BlendMode::Overlay => tiny_skia::BlendMode::Overlay,
        crate::svg_engine::usvg::BlendMode::Darken => tiny_skia::BlendMode::Darken,
        crate::svg_engine::usvg::BlendMode::Lighten => tiny_skia::BlendMode::Lighten,
        crate::svg_engine::usvg::BlendMode::ColorDodge => tiny_skia::BlendMode::ColorDodge,
        crate::svg_engine::usvg::BlendMode::ColorBurn => tiny_skia::BlendMode::ColorBurn,
        crate::svg_engine::usvg::BlendMode::HardLight => tiny_skia::BlendMode::HardLight,
        crate::svg_engine::usvg::BlendMode::SoftLight => tiny_skia::BlendMode::SoftLight,
        crate::svg_engine::usvg::BlendMode::Difference => tiny_skia::BlendMode::Difference,
        crate::svg_engine::usvg::BlendMode::Exclusion => tiny_skia::BlendMode::Exclusion,
        crate::svg_engine::usvg::BlendMode::Hue => tiny_skia::BlendMode::Hue,
        crate::svg_engine::usvg::BlendMode::Saturation => tiny_skia::BlendMode::Saturation,
        crate::svg_engine::usvg::BlendMode::Color => tiny_skia::BlendMode::Color,
        crate::svg_engine::usvg::BlendMode::Luminosity => tiny_skia::BlendMode::Luminosity,
        crate::svg_engine::usvg::BlendMode::PlusLighter => tiny_skia::BlendMode::Plus,
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn oversized_isolated_layer_leaves_the_destination_unchanged()
    -> Result<(), Box<dyn std::error::Error>> {
        use tiny_skia::{IntRect, Transform};

        let tree = oversized_layer_tree()?;
        let group = fixture_group(&tree, "oversized").ok_or("isolated group missing")?;
        let context = super::Context {
            max_bbox: IntRect::from_xywh(0, 0, 600_000_000, 8).ok_or("invalid test bounds")?,
        };
        let mut image = blue_layer_fixture().ok_or("pixmap unavailable")?;
        let expected = image.data().to_vec();
        // WHY: tiny-skia は行のバイト数を i32 で表せない領域を、確保前に拒否する。
        let transform = Transform::from_scale(75_000_000.0, 1.0);
        let bounds = group
            .layer_bounding_box()
            .transform(transform)
            .and_then(|bbox| super::layer_bbox(group, &context, bbox))
            .ok_or("invalid integer layer bbox")?;
        assert!(bounds.width() > i32::MAX as u32 / 4);
        assert!(super::render_layer(group, &context, transform).is_none());
        assert!(super::render_group(group, &context, transform, &mut image.as_mut()).is_none());
        super::render_nodes(tree.root(), &context, transform, &mut image.as_mut());
        assert_eq!(image.data(), expected);
        Ok(())
    }

    #[test]
    fn isolated_layer_fixture_requires_a_group() -> Result<(), crate::svg_engine::usvg::Error> {
        let tree = oversized_layer_tree()?;
        assert!(matches!(
            tree.node_by_id("shape"),
            Some(crate::svg_engine::usvg::Node::Path(_))
        ));
        assert!(fixture_group(&tree, "shape").is_none());
        Ok(())
    }

    fn fixture_group<'a>(
        tree: &'a crate::svg_engine::usvg::Tree,
        id: &str,
    ) -> Option<&'a crate::svg_engine::usvg::Group> {
        tree.node_by_id(id).and_then(|node| match node {
            crate::svg_engine::usvg::Node::Group(group) => Some(group.as_ref()),
            _ => None,
        })
    }

    fn oversized_layer_tree() -> Result<crate::svg_engine::usvg::Tree, crate::svg_engine::usvg::Error> {
        let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'><g id='oversized' opacity='.5'><rect id='shape' width='8' height='8'/></g></svg>";
        crate::svg_engine::usvg::Tree::from_str(svg, &crate::svg_engine::usvg::Options::default())
    }

    fn blue_layer_fixture() -> Option<tiny_skia::Pixmap> {
        let mut image = tiny_skia::Pixmap::new(8, 8)?;
        image.fill(tiny_skia::Color::from_rgba8(0, 0, 255, 255));
        Some(image)
    }

    // Derived from https://github.com/servo/servo/issues/42258.
    #[test]
    fn filter_bbox_outside_int_rect() {
        let svg = r#"<svg filter="url(#f)"><filter id="f" x="2em"><feFlood/></filter><path d="M0 0H1e8V1"/></svg>"#;
        let tree = crate::svg_engine::usvg::Tree::from_str(svg, &crate::svg_engine::usvg::Options::default()).unwrap();
        let mut pixmap = tiny_skia::Pixmap::new(1, 1).unwrap();

        // Just make sure we don't panic.
        crate::svg_engine::resvg::render(
            &tree,
            tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
    }
}
