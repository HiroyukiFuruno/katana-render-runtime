// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::converter::{self, SvgColorExt};
use super::paint_server;
use super::svgtree::{AId, FromValue, SvgNode};
use crate::svg_engine::usvg::tree::ContextElement;
use crate::svg_engine::usvg::{
    ApproxEqUlps, Color, Fill, FillRule, LineCap, LineJoin, Opacity, Paint, Stroke,
    StrokeMiterlimit, Units,
};

impl<'a, 'input: 'a> FromValue<'a, 'input> for LineCap {
    fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
        match value {
            "butt" => Some(LineCap::Butt),
            "round" => Some(LineCap::Round),
            "square" => Some(LineCap::Square),
            _ => None,
        }
    }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for LineJoin {
    fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
        match value {
            "miter" => Some(LineJoin::Miter),
            "miter-clip" => Some(LineJoin::MiterClip),
            "round" => Some(LineJoin::Round),
            "bevel" => Some(LineJoin::Bevel),
            _ => None,
        }
    }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for FillRule {
    fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
        match value {
            "nonzero" => Some(FillRule::NonZero),
            "evenodd" => Some(FillRule::EvenOdd),
            _ => None,
        }
    }
}

pub(crate) fn resolve_fill(
    node: SvgNode,
    has_bbox: bool,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Option<Fill> {
    if state.parent_clip_path.is_some() {
        return Some(clip_fill(node));
    }

    let mut sub_opacity = Opacity::ONE;
    let (paint, context_element) = inherited_paint(node, AId::Fill, has_bbox, state, &mut sub_opacity, cache)?;

    let fill_opacity = node
        .find_attribute::<Opacity>(AId::FillOpacity)
        .unwrap_or(Opacity::ONE);

    Some(Fill {
        paint,
        opacity: sub_opacity * fill_opacity,
        rule: node.find_attribute(AId::FillRule).unwrap_or_default(),
        context_element,
    })
}

fn clip_fill(node: SvgNode) -> Fill {
    // A `clipPath` child can be filled only with a black color.
    Fill {
        paint: Paint::Color(Color::black()),
        opacity: Opacity::ONE,
        rule: node.find_attribute(AId::ClipRule).unwrap_or_default(),
        context_element: None,
    }
}

pub(crate) fn resolve_stroke(
    node: SvgNode,
    has_bbox: bool,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Option<Stroke> {
    if state.parent_clip_path.is_some() {
        // A `clipPath` child cannot be stroked.
        return None;
    }

    let mut sub_opacity = Opacity::ONE;
    let (paint, context_element) = inherited_paint(node, AId::Stroke, has_bbox, state, &mut sub_opacity, cache)?;
    make_stroke(node, state, paint, context_element, sub_opacity)
}

fn make_stroke(
    node: SvgNode,
    state: &converter::State,
    paint: Paint,
    context_element: Option<ContextElement>,
    sub_opacity: Opacity,
) -> Option<Stroke> {
    let width = node.resolve_valid_length(AId::StrokeWidth, state, 1.0)?;

    // Must be bigger than 1.
    let miterlimit = resolve_miterlimit(node);

    let stroke_opacity = node
        .find_attribute::<Opacity>(AId::StrokeOpacity)
        .unwrap_or(Opacity::ONE);

    let stroke = Stroke {
        paint,
        dasharray: conv_dasharray(node, state),
        dashoffset: node.resolve_length(AId::StrokeDashoffset, state, 0.0),
        miterlimit,
        opacity: sub_opacity * stroke_opacity,
        width,
        linecap: node.find_attribute(AId::StrokeLinecap).unwrap_or_default(),
        linejoin: node.find_attribute(AId::StrokeLinejoin).unwrap_or_default(),
        context_element,
    };

    Some(stroke)
}

fn resolve_miterlimit(node: SvgNode) -> StrokeMiterlimit {
    let miterlimit = node.find_attribute(AId::StrokeMiterlimit).unwrap_or(4.0);
    let miterlimit = if miterlimit < 1.0 { 1.0 } else { miterlimit };
    StrokeMiterlimit::new(miterlimit)
}

fn inherited_paint(
    node: SvgNode,
    aid: AId,
    has_bbox: bool,
    state: &converter::State,
    opacity: &mut Opacity,
    cache: &mut converter::Cache,
) -> Option<(Paint, Option<ContextElement>)> {
    if let Some(n) = node.ancestors().find(|n| n.has_attribute(aid)) {
        let value: &str = n.attribute(aid)?;
        convert_paint(node, value, aid, has_bbox, state, opacity, cache)
    } else if aid == AId::Fill {
        Some((Paint::Color(Color::black()), None))
    } else {
        None
    }
}

fn convert_paint(
    node: SvgNode,
    value: &str,
    aid: AId,
    has_bbox: bool,
    state: &converter::State,
    opacity: &mut Opacity,
    cache: &mut converter::Cache,
) -> Option<(Paint, Option<ContextElement>)> {
    let paint = parse_svg_paint(value, aid)?;
    match paint {
        // 継承は属性ツリーで解決済み。
        svgtypes::Paint::None | svgtypes::Paint::Inherit => None,
        svgtypes::Paint::ContextFill => context_fill(state),
        svgtypes::Paint::ContextStroke => context_stroke(state),
        svgtypes::Paint::CurrentColor => Some(color_paint(current_color(node), opacity)),
        svgtypes::Paint::Color(svg_color) => Some(color_paint(svg_color, opacity)),
        svgtypes::Paint::FuncIRI(func_iri, fallback) => {
            convert_paint_link(node, (func_iri, fallback), aid, has_bbox, state, opacity, cache)
        }
    }
}

fn context_fill(state: &converter::State) -> Option<(Paint, Option<ContextElement>)> {
    state.context_element.clone().and_then(|(f, _)| f).map(|f| (f.paint, f.context_element))
}

fn context_stroke(state: &converter::State) -> Option<(Paint, Option<ContextElement>)> {
    state.context_element.clone().and_then(|(_, s)| s).map(|s| (s.paint, s.context_element))
}

fn current_color(node: SvgNode) -> svgtypes::Color {
    node.find_attribute(AId::Color).unwrap_or_else(svgtypes::Color::black)
}

fn parse_svg_paint(value: &str, aid: AId) -> Option<svgtypes::Paint<'_>> {
    let paint = match svgtypes::Paint::from_str(value) {
        Ok(v) => v,
        Err(_) => {
            if aid == AId::Fill {
                log::warn!(
                    "Failed to parse fill value: '{}'. Fallback to black.",
                    value
                );
                svgtypes::Paint::Color(svgtypes::Color::black())
            } else {
                log::warn!(
                    "Failed to parse stroke value: '{}'. Fallback to no stroke.",
                    value
                );
                return None;
            }
        }
    };

    Some(paint)
}

fn color_paint(color: svgtypes::Color, opacity: &mut Opacity) -> (Paint, Option<ContextElement>) {
    let (color, alpha) = color.split_alpha();
    *opacity = alpha;
    (Paint::Color(color), None)
}

fn convert_paint_link(
    node: SvgNode,
    link: (&str, Option<svgtypes::PaintFallback>),
    aid: AId,
    has_bbox: bool,
    state: &converter::State,
    opacity: &mut Opacity,
    cache: &mut converter::Cache,
) -> Option<(Paint, Option<ContextElement>)> {
    let (func_iri, fallback) = link;
            if let Some(link) = node.document().element_by_id(func_iri) {
                let tag_name = link.tag_name().unwrap();
                if let Some(kind) = paint_server::PaintServerKind::from_element(tag_name) {
                    let resolved = paint_server::convert(link, kind, state, cache);
                    paint_server_result(node, fallback, has_bbox, opacity, resolved)
                } else {
                    log::warn!("'{}' cannot be used to {} a shape.", tag_name, aid);
                    None
                }
            } else {
                from_fallback(node, fallback, opacity).map(|p| (p, None))
            }
}

fn paint_server_result(
    node: SvgNode,
    fallback: Option<svgtypes::PaintFallback>,
    has_bbox: bool,
    opacity: &mut Opacity,
    resolved: Option<paint_server::ServerOrColor>,
) -> Option<(Paint, Option<ContextElement>)> {
    match resolved {
                        Some(paint_server::ServerOrColor::Server(paint)) => {
                            // We can use a paint server node with ObjectBoundingBox units
                            // for painting only when the shape itself has a bbox.
                            //
                            // See SVG spec 7.11 for details.

                            if !has_bbox && paint.units() == Units::ObjectBoundingBox {
                                from_fallback(node, fallback, opacity).map(|p| (p, None))
                            } else {
                                Some((paint, None))
                            }
                        }
                        Some(paint_server::ServerOrColor::Color { color, opacity: so }) => {
                            *opacity = so;
                            Some((Paint::Color(color), None))
                        }
                        None => from_fallback(node, fallback, opacity).map(|p| (p, None)),
    }
}

fn from_fallback(
    node: SvgNode,
    fallback: Option<svgtypes::PaintFallback>,
    opacity: &mut Opacity,
) -> Option<Paint> {
    match fallback? {
        svgtypes::PaintFallback::None => None,
        svgtypes::PaintFallback::CurrentColor => {
            let svg_color: svgtypes::Color = node
                .find_attribute(AId::Color)
                .unwrap_or_else(svgtypes::Color::black);
            let (color, alpha) = svg_color.split_alpha();
            *opacity = alpha;
            Some(Paint::Color(color))
        }
        svgtypes::PaintFallback::Color(svg_color) => {
            let (color, alpha) = svg_color.split_alpha();
            *opacity = alpha;
            Some(Paint::Color(color))
        }
    }
}

// Prepare the 'stroke-dasharray' according to:
// https://www.w3.org/TR/SVG11/painting.html#StrokeDasharrayProperty
fn conv_dasharray(node: SvgNode, state: &converter::State) -> Option<Vec<f32>> {
    let node = node
        .ancestors()
        .find(|n| n.has_attribute(AId::StrokeDasharray))?;
    let list = super::units::convert_list(node, AId::StrokeDasharray, state)?;

    // `A negative value is an error`
    if list.iter().any(|n| n.is_sign_negative()) {
        return None;
    }

    // `If the sum of the values is zero, then the stroke is rendered
    // as if a value of none were specified.`
    if dasharray_is_zero(&list) {
        return None;
    }

    // `If an odd number of values is provided, then the list of values
    // is repeated to yield an even number of values.`
    if list.len() % 2 != 0 {
        let mut tmp_list = list.clone();
        tmp_list.extend_from_slice(&list);
        return Some(tmp_list);
    }

    Some(list)
}

fn dasharray_is_zero(list: &[f32]) -> bool {
    // no Iter::sum(), because of f64
    let mut sum: f32 = 0.0;
    for n in list.iter() {
        sum += *n;
    }
    sum.approx_eq_ulps(&0.0, 4)
}
