// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::str::FromStr;
use std::sync::Arc;

use strict_num::PositiveF32;
use svgtypes::{Length, LengthUnit as Unit};

use super::OptionLog;
use super::converter::{self, Cache, SvgColorExt};
use super::svgtree::{AId, EId, SvgNode};
use crate::svg_engine::usvg::*;

pub(crate) enum ServerOrColor {
    Server(Paint),
    Color { color: Color, opacity: Opacity },
}

pub(super) enum PaintServerKind {
    LinearGradient,
    RadialGradient,
    Pattern,
}

impl PaintServerKind {
    pub(super) fn from_element(tag: EId) -> Option<Self> {
        match tag {
            EId::LinearGradient => Some(Self::LinearGradient),
            EId::RadialGradient => Some(Self::RadialGradient),
            EId::Pattern => Some(Self::Pattern),
            _ => None,
        }
    }
}

pub(super) fn convert(
    node: SvgNode,
    kind: PaintServerKind,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Option<ServerOrColor> {
    // Check for existing.
    if let Some(paint) = cache.paint.get(node.element_id()) {
        return Some(ServerOrColor::Server(paint.clone()));
    }

    let paint = match kind {
        PaintServerKind::LinearGradient => convert_linear(node, state),
        PaintServerKind::RadialGradient => convert_radial(node, state),
        PaintServerKind::Pattern => convert_pattern(node, state, cache),
    };

    if let Some(ServerOrColor::Server(paint)) = &paint {
        cache
            .paint
            .insert(node.element_id().to_string(), paint.clone());
    }

    paint
}

#[inline(never)]
fn convert_linear(node: SvgNode, state: &converter::State) -> Option<ServerOrColor> {
    let id = NonEmptyString::new(node.element_id().to_string())?;

    let stops = convert_stops(find_gradient_with_stops(node)?);
    if let [stop] = stops.as_slice() {
        return Some(stop_to_color(stop));
    }

    let units = convert_units(node, AId::GradientUnits, Units::ObjectBoundingBox);
    let transform =
        resolve_attr(node, AId::GradientTransform).resolve_transform(AId::GradientTransform, state);

    let (x1, y1, x2, y2) = linear_gradient_coordinates(node, units, state);
    let gradient = LinearGradient {
        x1,
        y1,
        x2,
        y2,
        base: BaseGradient {
            id,
            units,
            transform,
            spread_method: convert_spread_method(node),
            stops,
        },
    };

    Some(ServerOrColor::Server(Paint::LinearGradient(Arc::new(
        gradient,
    ))))
}

fn linear_gradient_coordinates(
    node: SvgNode,
    units: Units,
    state: &converter::State,
) -> (f32, f32, f32, f32) {
    let x1 = resolve_number(node, AId::X1, units, state, Length::zero());
    let y1 = resolve_number(node, AId::Y1, units, state, Length::zero());
    let x2 = resolve_number(node, AId::X2, units, state, Length::new(100.0, Unit::Percent));
    let y2 = resolve_number(node, AId::Y2, units, state, Length::zero());
    (x1, y1, x2, y2)
}

#[inline(never)]
fn convert_radial(node: SvgNode, state: &converter::State) -> Option<ServerOrColor> {
    let id = NonEmptyString::new(node.element_id().to_string())?;

    let stops = convert_stops(find_gradient_with_stops(node)?);
    if let [stop] = stops.as_slice() {
        return Some(stop_to_color(stop));
    }

    let units = convert_units(node, AId::GradientUnits, Units::ObjectBoundingBox);
    let r = resolve_number(node, AId::R, units, state, Length::new(50.0, Unit::Percent));
    let fr = resolve_number(node, AId::Fr, units, state, Length::zero());

    // 'A value of zero will cause the area to be painted as a single color
    // using the color and opacity of the last gradient stop.'
    //
    // https://www.w3.org/TR/SVG11/pservers.html#RadialGradientElementRAttribute
    if !r.is_valid_length() {
        let stop = stops.last().unwrap();
        return Some(ServerOrColor::Color {
            color: stop.color,
            opacity: stop.opacity,
        });
    }

    Some(make_radial_gradient(node, state, id, stops, units, r, fr))
}

fn make_radial_gradient(
    node: SvgNode,
    state: &converter::State,
    id: NonEmptyString,
    stops: Vec<Stop>,
    units: Units,
    r: f32,
    fr: f32,
) -> ServerOrColor {
    let spread_method = convert_spread_method(node);
    let (cx, cy, fx, fy) = radial_gradient_coordinates(node, units, state);
    let transform =
        resolve_attr(node, AId::GradientTransform).resolve_transform(AId::GradientTransform, state);

    let gradient = RadialGradient {
        cx,
        cy,
        r: PositiveF32::new(r).unwrap(),
        fx,
        fy,
        fr: PositiveF32::new(fr).unwrap_or(PositiveF32::ZERO),
        base: BaseGradient {
            id,
            units,
            transform,
            spread_method,
            stops,
        },
    };

    ServerOrColor::Server(Paint::RadialGradient(Arc::new(gradient)))
}

fn radial_gradient_coordinates(
    node: SvgNode,
    units: Units,
    state: &converter::State,
) -> (f32, f32, f32, f32) {
    let cx = resolve_number(
        node,
        AId::Cx,
        units,
        state,
        Length::new(50.0, Unit::Percent),
    );
    let cy = resolve_number(
        node,
        AId::Cy,
        units,
        state,
        Length::new(50.0, Unit::Percent),
    );
    let fx = resolve_number(node, AId::Fx, units, state, Length::new_number(cx as f64));
    let fy = resolve_number(node, AId::Fy, units, state, Length::new_number(cy as f64));
    (cx, cy, fx, fy)
}

#[inline(never)]
fn convert_pattern(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Option<ServerOrColor> {
    let node_with_children = find_pattern_with_children(node)?;

    let id = NonEmptyString::new(node.element_id().to_string())?;

    let view_box = pattern_view_box(node);

    let units = convert_units(node, AId::PatternUnits, Units::ObjectBoundingBox);
    let content_units = convert_units(node, AId::PatternContentUnits, Units::UserSpaceOnUse);

    let transform =
        resolve_attr(node, AId::PatternTransform).resolve_transform(AId::PatternTransform, state);

    let rect = pattern_rect(node, units, state)?;

    let mut patt = Pattern {
        id,
        units,
        content_units,
        transform,
        rect,
        view_box,
        root: Group::empty(),
    };

    populate_pattern(&mut patt, node_with_children, state, cache)?;
    finish_pattern(&mut patt, cache);
    Some(ServerOrColor::Server(Paint::Pattern(Arc::new(patt))))
}

fn finish_pattern(patt: &mut Pattern, cache: &mut converter::Cache) {
    patt.root.calculate_bounding_boxes();
    update_paint_servers(&mut patt.root, Transform::default(), None, None, cache);
}

fn pattern_view_box(node: SvgNode) -> Option<ViewBox> {
    let n1 = resolve_attr(node, AId::ViewBox);
    let n2 = resolve_attr(node, AId::PreserveAspectRatio);
    n1.parse_viewbox().map(|vb| ViewBox {
        rect: vb,
        aspect: n2.attribute(AId::PreserveAspectRatio).unwrap_or_default(),
    })
}

fn pattern_rect(node: SvgNode, units: Units, state: &converter::State) -> Option<NonZeroRect> {
    let rect = NonZeroRect::from_xywh(
        resolve_number(node, AId::X, units, state, Length::zero()),
        resolve_number(node, AId::Y, units, state, Length::zero()),
        resolve_number(node, AId::Width, units, state, Length::zero()),
        resolve_number(node, AId::Height, units, state, Length::zero()),
    );
    rect.log_none(|| {
        log::warn!(
            "Pattern '{}' has an invalid size. Skipped.",
            node.element_id()
        );
    })
}

fn populate_pattern(
    patt: &mut Pattern,
    node_with_children: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Option<()> {
    // We can apply viewbox transform only for user space coordinates.
    // Otherwise we need a bounding box, which is unknown at this point.
    if let Some(view_box) = patt.view_box
        && patt.units == Units::UserSpaceOnUse
        && patt.content_units == Units::UserSpaceOnUse
    {
        let mut g = Group::empty();
        g.transform = view_box.to_transform(patt.rect.size());
        g.abs_transform = g.transform;

        converter::convert_children(node_with_children, state, cache, &mut g);
        if !g.has_children() {
            return None;
        }

        g.calculate_bounding_boxes();
        patt.root.children.push(Node::Group(Box::new(g)));
    } else {
        converter::convert_children(node_with_children, state, cache, &mut patt.root);
        if !patt.root.has_children() {
            return None;
        }
    }

    Some(())
}

fn convert_spread_method(node: SvgNode) -> SpreadMethod {
    let node = resolve_attr(node, AId::SpreadMethod);
    node.attribute(AId::SpreadMethod).unwrap_or_default()
}

pub(crate) fn convert_units(node: SvgNode, name: AId, def: Units) -> Units {
    let node = resolve_attr(node, name);
    node.attribute(name).unwrap_or(def)
}

fn find_gradient_with_stops<'a, 'input: 'a>(
    node: SvgNode<'a, 'input>,
) -> Option<SvgNode<'a, 'input>> {
    for link in node.href_iter() {
        if !link.tag_name().unwrap().is_gradient() {
            log::warn!(
                "Gradient '{}' cannot reference '{}' via 'xlink:href'.",
                node.element_id(),
                link.tag_name().unwrap()
            );
            return None;
        }

        if link.children().any(|n| n.tag_name() == Some(EId::Stop)) {
            return Some(link);
        }
    }

    None
}

fn find_pattern_with_children<'a, 'input: 'a>(
    node: SvgNode<'a, 'input>,
) -> Option<SvgNode<'a, 'input>> {
    for link in node.href_iter() {
        if link.tag_name() != Some(EId::Pattern) {
            log::warn!(
                "Pattern '{}' cannot reference '{}' via 'xlink:href'.",
                node.element_id(),
                link.tag_name().unwrap()
            );
            return None;
        }

        if link.has_children() {
            return Some(link);
        }
    }

    None
}

fn convert_stops(grad: SvgNode) -> Vec<Stop> {
    let mut stops = read_gradient_stops(grad);
    remove_middle_equal_stops(&mut stops);
    separate_zero_stops(&mut stops);
    shift_equal_stops(&mut stops);
    stops
}

fn read_gradient_stops(grad: SvgNode) -> Vec<Stop> {
    let mut stops = Vec::new();
    let mut prev_offset = Length::zero();
    for stop in grad.children() {
        if stop.tag_name() != Some(EId::Stop) {
            log::warn!("Invalid gradient child: '{:?}'.", stop.tag_name().unwrap());
            continue;
        }
        stops.push(read_gradient_stop(stop, &mut prev_offset));
    }
    stops
}

fn read_gradient_stop(stop: SvgNode, prev_offset: &mut Length) -> Stop {
    // `number` can be either a number or a percentage.
    let offset = stop.attribute(AId::Offset).unwrap_or(*prev_offset);
    let offset = match offset.unit {
        Unit::None => offset.number,
        Unit::Percent => offset.number / 100.0,
        _ => prev_offset.number,
    };
    *prev_offset = Length::new_number(offset);
    let offset = crate::svg_engine::usvg::f32_bound(0.0, offset as f32, 1.0);
    let (color, opacity) = gradient_stop_color(stop).split_alpha();
    let stop_opacity = stop
        .attribute::<Opacity>(AId::StopOpacity)
        .unwrap_or(Opacity::ONE);
    Stop {
        offset: StopOffset::new_clamped(offset),
        color,
        opacity: opacity * stop_opacity,
    }
}

fn gradient_stop_color(stop: SvgNode) -> svgtypes::Color {
    match stop.attribute(AId::StopColor) {
        Some("currentColor") => stop
            .find_attribute(AId::Color)
            .unwrap_or_else(svgtypes::Color::black),
        Some(value) => {
            if let Ok(c) = svgtypes::Color::from_str(value) {
                c
            } else {
                log::warn!("Failed to parse stop-color value: '{}'.", value);
                svgtypes::Color::black()
            }
        }
        _ => svgtypes::Color::black(),
    }
}

fn remove_middle_equal_stops(stops: &mut Vec<Stop>) {
    // Remove stops with equal offset.
    //
    // Example:
    // offset="0.5"
    // offset="0.7"
    // offset="0.7" <-- this one should be removed
    // offset="0.7"
    // offset="0.9"
    if stops.len() >= 3 {
        let mut i = 0;
        while i < stops.len() - 2 {
            let offset1 = stops[i + 0].offset.get();
            let offset2 = stops[i + 1].offset.get();
            let offset3 = stops[i + 2].offset.get();

            if offset1.approx_eq_ulps(&offset2, 4) && offset2.approx_eq_ulps(&offset3, 4) {
                // Remove offset in the middle.
                stops.remove(i + 1);
            } else {
                i += 1;
            }
        }
    }

}

fn separate_zero_stops(stops: &mut [Stop]) {
    // Remove zeros.
    //
    // From:
    // offset="0.0"
    // offset="0.0"
    // offset="0.7"
    //
    // To:
    // offset="0.0"
    // offset="0.00000001"
    // offset="0.7"
    if stops.len() >= 2 {
        let mut i = 0;
        while i < stops.len() - 1 {
            let offset1 = stops[i + 0].offset.get();
            let offset2 = stops[i + 1].offset.get();

            if offset1.approx_eq_ulps(&0.0, 4) && offset2.approx_eq_ulps(&0.0, 4) {
                stops[i + 1].offset = StopOffset::new_clamped(offset1 + f32::EPSILON);
            }

            i += 1;
        }
    }

}

fn shift_equal_stops(stops: &mut [Stop]) {
    // Shift equal offsets.
    //
    // From:
    // offset="0.5"
    // offset="0.7"
    // offset="0.7"
    //
    // To:
    // offset="0.5"
    // offset="0.699999999"
    // offset="0.7"
    {
        let mut i = 1;
        while i < stops.len() {
            let offset1 = stops[i - 1].offset.get();
            let offset2 = stops[i - 0].offset.get();

            // Next offset must be smaller then previous.
            if offset1 > offset2 || offset1.approx_eq_ulps(&offset2, 4) {
                // Make previous offset a bit smaller.
                let new_offset = offset1 - f32::EPSILON;
                stops[i - 1].offset = StopOffset::new_clamped(new_offset);
                stops[i - 0].offset = StopOffset::new_clamped(offset1);
            }

            i += 1;
        }
    }

}

#[inline(never)]
pub(crate) fn resolve_number(
    node: SvgNode,
    name: AId,
    units: Units,
    state: &converter::State,
    def: Length,
) -> f32 {
    resolve_attr(node, name).convert_length(name, units, state, def)
}

fn resolve_attr<'a, 'input: 'a>(node: SvgNode<'a, 'input>, name: AId) -> SvgNode<'a, 'input> {
    if node.has_attribute(name) {
        return node;
    }

    let owner = node.tag_name();
    for link in node.href_iter() {
        if !accepts_reference_attribute(owner, name, link.tag_name()) {
            break;
        }
        if link.has_attribute(name) {
            return link;
        }
    }
    node
}

fn accepts_reference_attribute(owner: Option<EId>, name: AId, target: Option<EId>) -> bool {
    use EId::{Filter, LinearGradient, Pattern, RadialGradient};

    matches!(
        (owner, name, target),
        // 座標の継承は同じ種類のグラデーションに限る。
        (Some(LinearGradient), AId::X1 | AId::Y1 | AId::X2 | AId::Y2, Some(LinearGradient))
            | (Some(RadialGradient), AId::Cx | AId::Cy | AId::R | AId::Fx | AId::Fy | AId::Fr, Some(RadialGradient))
            | (Some(LinearGradient | RadialGradient),
               AId::GradientUnits | AId::SpreadMethod | AId::GradientTransform,
               Some(LinearGradient | RadialGradient))
            | (Some(Pattern), _, Some(Pattern))
            | (Some(Filter), _, Some(Filter))
    )
}

fn stop_to_color(stop: &Stop) -> ServerOrColor {
    ServerOrColor::Color {
        color: stop.color,
        opacity: stop.opacity,
    }
}

// Update paints servers by doing the following:
// 1. Replace context fills/strokes that are linked to
// a use node with their actual values.
// 2. Convert all object units to UserSpaceOnUse
pub fn update_paint_servers(
    group: &mut Group,
    context_transform: Transform,
    context_bbox: Option<Rect>,
    text_bbox: Option<Rect>,
    cache: &mut Cache,
) {
    for child in &mut group.children {
        // Set context transform and bbox if applicable if the
        // current group is a use node.
        let (context_transform, context_bbox) = if group.is_context_element {
            (group.abs_transform, Some(group.bounding_box))
        } else {
            (context_transform, context_bbox)
        };

        node_to_user_coordinates(child, context_transform, context_bbox, text_bbox, cache);
    }
}

// When parsing clipPaths, masks and filters we already know group's bounding box.
// But with gradients and patterns we don't, because we have to know text bounding box
// before we even parsed it. Which is impossible.
// Therefore our only choice is to parse gradients and patterns preserving their units
// and then replace them with `userSpaceOnUse` after the whole tree parsing is finished.
// So while gradients and patterns do still store their units,
// they are not exposed in the public API and for the caller they are always `userSpaceOnUse`.
fn node_to_user_coordinates(
    node: &mut Node,
    context_transform: Transform,
    context_bbox: Option<Rect>,
    text_bbox: Option<Rect>,
    cache: &mut Cache,
) {
    match node {
        Node::Group(g) => {
            update_paint_servers(g, context_transform, context_bbox, text_bbox, cache);
        }
        Node::Path(path) => {
            // Paths inside `Text::flattened` are special and must use text's bounding box
            // instead of their own.
            let bbox = text_bbox.unwrap_or(path.bounding_box);
            let path_transform = path.abs_transform;
            process_path_paint(path, path_transform, context_transform, context_bbox, bbox, cache);
        }
        Node::Image(image) => {
            if let ImageKind::SVG(tree) = &mut image.kind {
                update_paint_servers(&mut tree.root, context_transform, context_bbox, None, cache);
            }
        }
        Node::Text(text) => {
            update_text_paint(text, context_transform, context_bbox, cache);
        }
    }
}

fn update_text_paint(
    text: &mut Text,
    context_transform: Transform,
    context_bbox: Option<Rect>,
    cache: &mut Cache,
) {
            // By the SVG spec, `tspan` doesn't have a bbox and uses the parent `text` bbox.
            // Therefore we have to use text's bbox when converting tspan and flatted text
            // paint servers.
            let bbox = text.bounding_box;

            // We need to update three things:
            // 1. The fills/strokes of the original elements in the usvg tree.
            // 2. The fills/strokes of the layouted elements of the text.
            // 3. The fills/strokes of the outlined text.

            process_original_text_paint(text, context_transform, context_bbox, bbox, cache);
            process_layouted_text_paint(text, context_transform, context_bbox, bbox, cache);
            update_paint_servers(
                &mut text.flattened,
                context_transform,
                context_bbox,
                Some(bbox),
                cache,
            );
}

fn process_original_text_paint(
    text: &mut Text,
    context_transform: Transform,
    context_bbox: Option<Rect>,
    bbox: Rect,
    cache: &mut Cache,
) {
            for chunk in &mut text.chunks {
                for span in &mut chunk.spans {
                    process_fill_and_stroke(
                        &mut span.fill,
                        &mut span.stroke,
                        text.abs_transform,
                        context_transform,
                        context_bbox,
                        bbox,
                        cache,
                    );
                    process_text_decoration(&mut span.decoration.underline, bbox, cache);
                    process_text_decoration(&mut span.decoration.overline, bbox, cache);
                    process_text_decoration(&mut span.decoration.line_through, bbox, cache);
                }
            }

}

fn process_layouted_text_paint(
    text: &mut Text,
    context_transform: Transform,
    context_bbox: Option<Rect>,
    bbox: Rect,
    cache: &mut Cache,
) {
            for span in &mut text.layouted {
                process_fill_and_stroke(
                    &mut span.fill,
                    &mut span.stroke,
                    text.abs_transform,
                    context_transform,
                    context_bbox,
                    bbox,
                    cache,
                );

                for path in [&mut span.overline, &mut span.underline, &mut span.line_through]
                    .into_iter()
                    .flatten()
                {
                        process_path_paint(
                            path,
                            text.abs_transform,
                            context_transform,
                            context_bbox,
                            bbox,
                            cache,
                        );
                }
            }

}

fn process_path_paint(
    path: &mut Path,
    path_transform: Transform,
    context_transform: Transform,
    context_bbox: Option<Rect>,
    bbox: Rect,
    cache: &mut Cache,
) {
    process_fill_and_stroke(
        &mut path.fill,
        &mut path.stroke,
        path_transform,
        context_transform,
        context_bbox,
        bbox,
        cache,
    );
}

fn process_fill_and_stroke(
    fill: &mut Option<Fill>,
    stroke: &mut Option<Stroke>,
    path_transform: Transform,
    context_transform: Transform,
    context_bbox: Option<Rect>,
    bbox: Rect,
    cache: &mut Cache,
) {
    process_fill(fill, path_transform, context_transform, context_bbox, bbox, cache);
    process_stroke(stroke, path_transform, context_transform, context_bbox, bbox, cache);
}

fn process_fill(
    fill: &mut Option<Fill>,
    path_transform: Transform,
    context_transform: Transform,
    context_bbox: Option<Rect>,
    bbox: Rect,
    cache: &mut Cache,
) {
    let mut ok = false;
    if let Some(fill) = fill.as_mut() {
        // Path context elements (i.e. for  markers) have already been resolved,
        // so we only care about use nodes.
        ok = process_paint(
            &mut fill.paint,
            matches!(fill.context_element, Some(ContextElement::UseNode)),
            context_transform,
            context_bbox,
            path_transform,
            bbox,
            cache,
        );
    }
    if !ok {
        *fill = None;
    }
}

fn process_stroke(
    stroke: &mut Option<Stroke>,
    path_transform: Transform,
    context_transform: Transform,
    context_bbox: Option<Rect>,
    bbox: Rect,
    cache: &mut Cache,
) {
    let mut ok = false;
    if let Some(stroke) = stroke.as_mut() {
        // Path context elements (i.e. for  markers) have already been resolved,
        // so we only care about use nodes.
        ok = process_paint(
            &mut stroke.paint,
            matches!(stroke.context_element, Some(ContextElement::UseNode)),
            context_transform,
            context_bbox,
            path_transform,
            bbox,
            cache,
        );
    }
    if !ok {
        *stroke = None;
    }
}

fn process_context_paint(
    paint: &mut Paint,
    context_transform: Transform,
    path_transform: Transform,
    cache: &mut Cache,
) -> Option<()> {
    // The idea is the following: We have a certain context element that has
    // a transform A, and further below in the tree we have for example a path
    // whose paint has a transform C. In order to get from A to C, there is some
    // transformation matrix B such that A x B = C. We now need to figure out
    // a way to get from C back to A, so that the transformation of the paint
    // matches the one from the context element, even if B was applied. How
    // do we do that? We calculate CxB^(-1), which will overall then have
    // the same effect as A. How do we calculate B^(-1)?
    // --> (A^(-1)xC)^(-1)
    let rev_transform = context_transform
        .invert()?
        .pre_concat(path_transform)
        .invert()?;

    match paint {
        Paint::Color(_) => {}
        Paint::LinearGradient(lg) => *paint = context_linear_gradient(lg, rev_transform, cache),
        Paint::RadialGradient(rg) => *paint = context_radial_gradient(rg, rev_transform, cache),
        Paint::Pattern(pat) => *paint = context_pattern(pat, rev_transform, cache),
    }

    Some(())
}

fn context_linear_gradient(lg: &LinearGradient, rev_transform: Transform, cache: &mut Cache) -> Paint {
    let transform = lg.transform.post_concat(rev_transform);
    Paint::LinearGradient(Arc::new(LinearGradient {
                x1: lg.x1,
                y1: lg.y1,
                x2: lg.x2,
                y2: lg.y2,
                base: BaseGradient {
                    id: cache.gen_linear_gradient_id(),
                    units: lg.units,
                    transform,
                    spread_method: lg.spread_method,
                    stops: lg.stops.clone(),
                },
    }))
}

fn context_radial_gradient(rg: &RadialGradient, rev_transform: Transform, cache: &mut Cache) -> Paint {
    let transform = rg.transform.post_concat(rev_transform);
    Paint::RadialGradient(Arc::new(RadialGradient {
                cx: rg.cx,
                cy: rg.cy,
                r: rg.r,
                fx: rg.fx,
                fy: rg.fy,
                fr: rg.fr,
                base: BaseGradient {
                    id: cache.gen_radial_gradient_id(),
                    units: rg.units,
                    transform,
                    spread_method: rg.spread_method,
                    stops: rg.stops.clone(),
                },
    }))
}

fn context_pattern(pat: &Pattern, rev_transform: Transform, cache: &mut Cache) -> Paint {
    let transform = pat.transform.post_concat(rev_transform);
    Paint::Pattern(Arc::new(Pattern {
                id: cache.gen_pattern_id(),
                units: pat.units,
                content_units: pat.content_units,
                transform,
                rect: pat.rect,
                view_box: pat.view_box,
                root: pat.root.clone(),
    }))
}

pub(crate) fn process_paint(
    paint: &mut Paint,
    has_context: bool,
    context_transform: Transform,
    context_bbox: Option<Rect>,
    path_transform: Transform,
    bbox: Rect,
    cache: &mut Cache,
) -> bool {
    if let Some(box_paint) = BoundingBoxPaint::from_paint(paint) {
        let bbox = if has_context {
            let Some(bbox) = context_bbox else {
                return false;
            };
            bbox
        } else {
            bbox
        };

        if box_paint.to_user_coordinates(bbox, cache).is_none() {
            return false;
        }
    }

    if has_context {
        process_context_paint(paint, context_transform, path_transform, cache);
    }

    true
}

fn process_text_decoration(style: &mut Option<TextDecorationStyle>, bbox: Rect, cache: &mut Cache) {
    if let Some(style) = style.as_mut() {
        process_fill(
            &mut style.fill,
            Transform::default(),
            Transform::default(),
            None,
            bbox,
            cache,
        );
        process_stroke(
            &mut style.stroke,
            Transform::default(),
            Transform::default(),
            None,
            bbox,
            cache,
        );
    }
}

enum BoundingBoxPaint<'a> {
    Linear(&'a mut Arc<LinearGradient>),
    Radial(&'a mut Arc<RadialGradient>),
    Pattern(&'a mut Arc<Pattern>),
}

impl<'a> BoundingBoxPaint<'a> {
    fn from_paint(paint: &'a mut Paint) -> Option<Self> {
        let uses_box_units = paint.units() == Units::ObjectBoundingBox;
        match paint {
            Paint::LinearGradient(gradient) if uses_box_units => {
                Some(Self::Linear(gradient))
            }
            Paint::RadialGradient(gradient) if uses_box_units => {
                Some(Self::Radial(gradient))
            }
            Paint::Pattern(pattern)
                if uses_box_units || pattern.content_units == Units::ObjectBoundingBox =>
            {
                Some(Self::Pattern(pattern))
            }
            _ => None,
        }
    }

    fn to_user_coordinates(self, bbox: Rect, cache: &mut Cache) -> Option<()> {
        let name = if matches!(self, Self::Pattern(_)) {
            "Pattern"
        } else {
            "Gradient"
        };
        let bbox = bbox
            .to_non_zero_rect()
            .log_none(|| log::warn!("{} on zero-sized shapes is not allowed.", name))?;

        // `Arc::get_mut()` allow us to modify some paint servers in-place.
        // This reduces the amount of cloning and preserves the original ID as well.
        match self {
            Self::Linear(lg) => linear_to_user_coordinates(lg, bbox, cache),
            Self::Radial(rg) => radial_to_user_coordinates(rg, bbox, cache),
            Self::Pattern(patt) => pattern_to_user_coordinates(patt, bbox, cache),
        }

        Some(())
    }
}

fn linear_to_user_coordinates(lg: &mut Arc<LinearGradient>, bbox: NonZeroRect, cache: &mut Cache) {
                let transform = lg.transform.post_concat(Transform::from_bbox(bbox));
                if let Some(lg) = Arc::get_mut(lg) {
                    lg.base.transform = transform;
                    lg.base.units = Units::UserSpaceOnUse;
                } else {
                    *lg = Arc::new(LinearGradient {
                        x1: lg.x1,
                        y1: lg.y1,
                        x2: lg.x2,
                        y2: lg.y2,
                        base: BaseGradient {
                            id: cache.gen_linear_gradient_id(),
                            units: Units::UserSpaceOnUse,
                            transform,
                            spread_method: lg.spread_method,
                            stops: lg.stops.clone(),
                        },
                    });
                }
}

fn radial_to_user_coordinates(rg: &mut Arc<RadialGradient>, bbox: NonZeroRect, cache: &mut Cache) {
                let transform = rg.transform.post_concat(Transform::from_bbox(bbox));
                if let Some(rg) = Arc::get_mut(rg) {
                    rg.base.transform = transform;
                    rg.base.units = Units::UserSpaceOnUse;
                } else {
                    *rg = Arc::new(RadialGradient {
                        cx: rg.cx,
                        cy: rg.cy,
                        r: rg.r,
                        fx: rg.fx,
                        fy: rg.fy,
                        fr: rg.fr,
                        base: BaseGradient {
                            id: cache.gen_radial_gradient_id(),
                            units: Units::UserSpaceOnUse,
                            transform,
                            spread_method: rg.spread_method,
                            stops: rg.stops.clone(),
                        },
                    });
                }
}

fn pattern_to_user_coordinates(patt: &mut Arc<Pattern>, bbox: NonZeroRect, cache: &mut Cache) {
                let rect = if patt.units == Units::ObjectBoundingBox {
                    patt.rect.bbox_transform(bbox)
                } else {
                    patt.rect
                };

                if let Some(patt) = Arc::get_mut(patt) {
                    update_unique_pattern(patt, bbox, rect);
                } else {
                    update_shared_pattern(patt, bbox, rect, cache);
                }
}

fn update_unique_pattern(patt: &mut Pattern, bbox: NonZeroRect, rect: NonZeroRect) {
                    patt.rect = rect;
                    patt.units = Units::UserSpaceOnUse;

                    if patt.content_units == Units::ObjectBoundingBox && patt.view_box.is_none() {
                        // No need to shift patterns.
                        let transform = Transform::from_scale(bbox.width(), bbox.height());
                        push_pattern_transform(&mut patt.root, transform);
                    }

                    if let Some(view_box) = patt.view_box {
                        push_pattern_transform(&mut patt.root, view_box.to_transform(rect.size()));
                    }

                    patt.content_units = Units::UserSpaceOnUse;
}

fn update_shared_pattern(
    patt: &mut Arc<Pattern>,
    bbox: NonZeroRect,
    rect: NonZeroRect,
    cache: &mut Cache,
) {
    let root = shared_pattern_root(patt, bbox, rect);
    *patt = Arc::new(Pattern {
        id: cache.gen_pattern_id(),
        units: Units::UserSpaceOnUse,
        content_units: Units::UserSpaceOnUse,
        transform: patt.transform,
        rect,
        view_box: patt.view_box,
        root,
    });
}

fn shared_pattern_root(patt: &Pattern, bbox: NonZeroRect, rect: NonZeroRect) -> Group {
                    let mut root = if patt.content_units == Units::ObjectBoundingBox
                        && patt.view_box.is_none()
                    {
                        // No need to shift patterns.
                        let transform = Transform::from_scale(bbox.width(), bbox.height());

                        let mut g = patt.root.clone();
                        push_pattern_transform(&mut g, transform);
                        g
                    } else {
                        patt.root.clone()
                    };

                    if let Some(view_box) = patt.view_box {
                        push_pattern_transform(&mut root, view_box.to_transform(rect.size()));
                    }

    root
}

#[cfg(test)]
crate::svg_engine::owned_svg_model_tests!("../../../../src/svg_engine_paint_server_model_tests.rs");

fn push_pattern_transform(root: &mut Group, transform: Transform) {
    // TODO: we should update abs_transform in all descendants as well
    let mut g = std::mem::replace(root, Group::empty());
    g.transform = transform;
    g.abs_transform = transform;

    root.children.push(Node::Group(Box::new(g)));
    root.calculate_bounding_boxes();
}

impl Paint {
    #[inline]
    pub(crate) fn units(&self) -> Units {
        match self {
            Self::Color(_) => Units::UserSpaceOnUse,
            Self::LinearGradient(lg) => lg.units,
            Self::RadialGradient(rg) => rg.units,
            Self::Pattern(patt) => patt.units,
        }
    }
}
