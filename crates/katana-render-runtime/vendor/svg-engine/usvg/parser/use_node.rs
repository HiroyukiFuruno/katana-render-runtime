// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use svgtypes::{Length, LengthUnit};

use super::svgtree::{AId, EId, SvgNode};
use super::{converter, style};
use crate::svg_engine::usvg::tree::ContextElement;
use crate::svg_engine::usvg::{
    ClipPath, Group, Node, NonZeroRect, Path, Size, Transform, ViewBox,
};

pub(crate) fn convert(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
    parent: &mut Group,
) {
    let child = match node.first_child() {
        Some(v) => v,
        None => return,
    };

    if state.parent_clip_path.is_some() && child.tag_name() == Some(EId::Symbol) {
        // Ignore `symbol` referenced by `use` inside a `clipPath`.
        // It will be ignored later anyway, but this will prevent
        // a redundant `clipPath` creation (which is required for `symbol`).
        return;
    }

    let mut use_state = use_node_state(node, state, cache);
    convert_use_reference(node, child, state, &mut use_state, cache, parent);
}

fn convert_use_reference(
    node: SvgNode,
    child: SvgNode,
    state: &converter::State,
    use_state: &mut converter::State,
    cache: &mut converter::Cache,
    parent: &mut Group,
) {
    let original = node.resolve_transform(AId::Transform, state);
    let translation = use_node_translation(node, use_state);
    if child.tag_name() == Some(EId::Symbol) {
        convert_symbol(node, child, original, translation, use_state, cache, parent);
    } else {
        let transform = original.pre_concat(translation);
        convert_regular_use(node, child, transform, use_state, cache, parent);
    }
}

fn use_node_state<'a>(
    node: SvgNode,
    state: &converter::State<'a>,
    cache: &mut converter::Cache,
) -> converter::State<'a> {
    let mut use_state = state.clone();
    use_state.context_element = Some((
        style::resolve_fill(node, true, state, cache).map(|mut f| {
            f.context_element = Some(ContextElement::UseNode);
            f
        }),
        style::resolve_stroke(node, true, state, cache).map(|mut s| {
            s.context_element = Some(ContextElement::UseNode);
            s
        }),
    ));

    use_state
}

fn use_node_translation(node: SvgNode, state: &converter::State) -> Transform {
    let x = node.convert_user_length(AId::X, state, Length::zero());
    let y = node.convert_user_length(AId::Y, state, Length::zero());
    Transform::default().pre_translate(x, y)
}

fn symbol_view_box(node: SvgNode, state: &converter::State) -> NonZeroRect {
    // symbol内の相対寸法は参照元useの寸法を基準にする。
    let def = Length::new(100.0, LengthUnit::Percent);
    let x = state.view_box.x();
    let y = state.view_box.y();
    let width = if node.has_attribute(AId::Width) {
        node.convert_user_length(AId::Width, state, def)
    } else {
        state.view_box.width()
    };
    let height = if node.has_attribute(AId::Height) {
        node.convert_user_length(AId::Height, state, def)
    } else {
        state.view_box.height()
    };
    NonZeroRect::from_xywh(x, y, width, height).unwrap_or(state.view_box)
}

fn convert_symbol(
    node: SvgNode,
    child: SvgNode,
    original: Transform,
    mut transform: Transform,
    state: &mut converter::State,
    cache: &mut converter::Cache,
    parent: &mut Group,
) {
    state.view_box = symbol_view_box(node, state);
    if let Some(viewbox) = viewbox_transform(node, child, state) {
        transform = transform.pre_concat(viewbox);
    }
    if let Some(rect) = get_clip_rect(node, child, state) {
        let mut group = clip_element(node, rect, original, state, cache);
        group.abs_transform = parent.abs_transform;
        populate_symbol_clip(node, child, transform, state, cache, &mut group);
        append_nonempty_symbol_clip(group, parent);
    } else {
        let transform = original.pre_concat(transform);
        append_symbol_group(node, child, transform, state, cache, parent);
    }
}

fn populate_symbol_clip(
    node: SvgNode,
    child: SvgNode,
    transform: Transform,
    state: &converter::State,
    cache: &mut converter::Cache,
    group: &mut Group,
) {
    if let Some(mut child_group) =
        converter::convert_group(node, state, true, cache, group, &|cache, child_group| {
            convert_children(child, transform, state, cache, false, child_group);
        })
    {
        // clip用groupに設定済みの変換とIDを子へ重複適用しない。
        group.is_context_element = true;
        child_group.id = String::new();
        child_group.transform = Transform::default();
        group.children.push(Node::Group(Box::new(child_group)));
    }
}

fn append_nonempty_symbol_clip(mut group: Group, parent: &mut Group) {
    if group.children.is_empty() {
        return;
    }
    group.calculate_bounding_boxes();
    parent.children.push(Node::Group(Box::new(group)));
}

fn append_symbol_group(
    node: SvgNode,
    child: SvgNode,
    transform: Transform,
    state: &converter::State,
    cache: &mut converter::Cache,
    parent: &mut Group,
) {
    if let Some(mut group) =
        converter::convert_group(node, state, false, cache, parent, &|cache, group| {
            convert_children(child, transform, state, cache, false, group);
        })
    {
        group.is_context_element = true;
        group.transform = Transform::default();
        parent.children.push(Node::Group(Box::new(group)));
    }
}

fn convert_regular_use(
    node: SvgNode,
    child: SvgNode,
    transform: Transform,
    state: &mut converter::State,
    cache: &mut converter::Cache,
    parent: &mut Group,
) {
    if child.tag_name() == Some(EId::Svg) {
        set_use_size(node, state);
    }
    convert_children(node, transform, state, cache, true, parent);
}

fn set_use_size(node: SvgNode, state: &mut converter::State) {
    let def = Length::new(100.0, LengthUnit::Percent);
    // 入れ子のuseでは外側の片軸指定を引き継がず、各軸を独立に設定する。
    state.use_size = (None, None);
    if node.has_attribute(AId::Width) {
        state.use_size.0 = Some(node.convert_user_length(AId::Width, state, def));
    }
    if node.has_attribute(AId::Height) {
        state.use_size.1 = Some(node.convert_user_length(AId::Height, state, def));
    }
}

pub(crate) fn convert_svg(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
    parent: &mut Group,
) {
    // We require original transformation to setup 'clipPath'.
    let mut orig_ts = node.resolve_transform(AId::Transform, state);
    let mut new_ts = Transform::default();

    let x = node.convert_user_length(AId::X, state, Length::zero());
    let y = node.convert_user_length(AId::Y, state, Length::zero());
    new_ts = new_ts.pre_translate(x, y);

    if let Some(ts) = viewbox_transform(node, node, state) {
        new_ts = new_ts.pre_concat(ts);
    }

    let new_state = nested_svg_state(node, x, y, state);

    if let Some(clip_rect) = get_clip_rect(node, node, state) {
        let mut g = clip_element(node, clip_rect, orig_ts, state, cache);
        g.abs_transform = parent.abs_transform.pre_concat(orig_ts);
        convert_children(node, new_ts, &new_state, cache, false, &mut g);
        g.calculate_bounding_boxes();
        parent.children.push(Node::Group(Box::new(g)));
    } else {
        orig_ts = orig_ts.pre_concat(new_ts);
        convert_children(node, orig_ts, &new_state, cache, false, parent);
    }
}

fn nested_svg_state<'a>(
    node: SvgNode,
    x: f32,
    y: f32,
    state: &converter::State<'a>,
) -> converter::State<'a> {
    let mut nested = state.clone();
    nested.view_box = if let Some(view_box) = node.parse_viewbox() {
        view_box
    } else {
        let (width, height) = use_node_size(node, state);
        let (width, height) = override_use_size(width, height, state);
        NonZeroRect::from_xywh(x, y, width, height).unwrap_or(state.view_box)
    };
    nested
}

fn clip_element(
    node: SvgNode,
    clip_rect: NonZeroRect,
    transform: Transform,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Group {
    // We can't set `clip-path` on the element itself,
    // because it will be affected by a possible transform.
    // So we have to create an additional group.

    let clip_path = clip_path_for_rect(clip_rect, cache);

    // Nodes generated by markers must not have an ID. Otherwise we would have duplicates.
    let id = if state.parent_markers.is_empty() {
        node.element_id().to_string()
    } else {
        String::new()
    };

    Group {
        id,
        transform,
        clip_path: Some(Arc::new(clip_path)),
        ..Group::empty()
    }
}

// Emulate a new viewport via clipPath.
//
// From:
// <defs/>
// <elem/>
//
// To:
// <defs>
//   <clipPath id="clipPath1">
//     <rect/>
//   </clipPath>
// </defs>
// <g clip-path="ulr(#clipPath1)">
//   <elem/>
// </g>

fn clip_path_for_rect(clip_rect: NonZeroRect, cache: &mut converter::Cache) -> ClipPath {
    let mut clip_path = ClipPath::empty(cache.gen_clip_path_id());

    let mut path = Path::new_simple(Arc::new(tiny_skia_path::PathBuilder::from_rect(
        clip_rect.to_rect(),
    )))
    .unwrap();
    path.fill = Some(crate::svg_engine::usvg::Fill::default());
    clip_path.root.children.push(Node::Path(Box::new(path)));
    clip_path.root.calculate_bounding_boxes();

    clip_path
}

fn convert_children(
    node: SvgNode,
    transform: Transform,
    state: &converter::State,
    cache: &mut converter::Cache,
    is_context_element: bool,
    parent: &mut Group,
) {
    // Cache the parent absolute transform.
    let parent_abs_transform = parent.abs_transform;

    let required = !transform.is_identity();

    let Some(g) = converter::convert_group(node, state, required, cache, parent, &|cache, g| {
        // Make sure they are correct before converting children.
        g.transform = transform;
        g.abs_transform = parent_abs_transform.pre_concat(transform);
        g.is_context_element = is_context_element;

        if state.parent_clip_path.is_some() {
            converter::convert_clip_path_elements(node, state, cache, g);
        } else {
            converter::convert_children(node, state, cache, g);
        }
    }) else {
        return;
    };

    parent.children.push(Node::Group(Box::new(g)));
}

fn get_clip_rect(
    use_node: SvgNode,
    symbol_node: SvgNode,
    state: &converter::State,
) -> Option<NonZeroRect> {
    if !requires_clip_rect(use_node, symbol_node, state) {
        return None;
    }
    let x = use_node.convert_user_length(AId::X, state, Length::zero());
    let y = use_node.convert_user_length(AId::Y, state, Length::zero());
    let (width, height) = viewport_size(use_node, state);
    NonZeroRect::from_xywh(x, y, width, height)
}

fn requires_clip_rect(use_node: SvgNode, symbol_node: SvgNode, state: &converter::State) -> bool {
    // No need to clip elements with overflow:visible.
    if matches!(
        symbol_node.attribute(AId::Overflow),
        Some("visible") | Some("auto")
    ) {
        return false;
    }

    // A nested `svg` with only the `viewBox` attribute and no "rectangle" (x, y, width, height)
    // should not be clipped.
    if use_node.tag_name() == Some(EId::Svg) {
        // Nested `svg` referenced by `use` still should be clipped, but by `use` bounds.
        if state.use_size.0.is_none() && state.use_size.1.is_none() {
            if !(use_node.has_attribute(AId::Width) && use_node.has_attribute(AId::Height)) {
                return false;
            }
        }
    }

    true
}

fn use_node_size(node: SvgNode, state: &converter::State) -> (f32, f32) {
    let def = Length::new(100.0, LengthUnit::Percent);
    let w = node.convert_user_length(AId::Width, state, def);
    let h = node.convert_user_length(AId::Height, state, def);
    (w, h)
}

fn viewbox_transform(
    node: SvgNode,
    linked: SvgNode,
    state: &converter::State,
) -> Option<Transform> {
    let (w, h) = viewport_size(node, state);

    let size = Size::from_wh(w, h)?;
    let rect = linked.parse_viewbox()?;
    let aspect = linked
        .attribute(AId::PreserveAspectRatio)
        .unwrap_or_default();
    let view_box = ViewBox { rect, aspect };

    Some(view_box.to_transform(size))
}

fn viewport_size(node: SvgNode, state: &converter::State) -> (f32, f32) {
    let (width, height) = use_node_size(node, state);
    if node.tag_name() == Some(EId::Svg) {
        override_use_size(width, height, state)
    } else {
        (width, height)
    }
}

fn override_use_size(width: f32, height: f32, state: &converter::State) -> (f32, f32) {
    // useの各軸指定を、参照するSVGの対応する寸法へ適用する。
    (
        state.use_size.0.unwrap_or(width),
        state.use_size.1.unwrap_or(height),
    )
}
