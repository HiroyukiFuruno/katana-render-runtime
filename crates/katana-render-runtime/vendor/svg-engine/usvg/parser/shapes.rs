// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use svgtypes::Length;
use tiny_skia_path::Path;

use super::svgtree::{AId, EId, SvgNode};
use super::{converter, units};
use crate::svg_engine::usvg::{ApproxEqUlps, IsValidLength, Rect};

pub(crate) fn convert(node: SvgNode, state: &converter::State) -> Option<Arc<Path>> {
    match node.tag_name()? {
        EId::Rect => convert_rect(node, state),
        EId::Circle => convert_circle(node, state),
        EId::Ellipse => convert_ellipse(node, state),
        EId::Line => convert_line(node, state),
        EId::Polyline => convert_polyline(node),
        EId::Polygon => convert_polygon(node),
        EId::Path => convert_path(node),
        _ => None,
    }
}

pub(crate) fn convert_path(node: SvgNode) -> Option<Arc<Path>> {
    let value: &str = node.attribute(AId::D)?;
    let mut builder = tiny_skia_path::PathBuilder::new();
    for segment in svgtypes::SimplifyingPathParser::from(value) {
        let segment = match segment {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Error during path parsing: {e}");
                break;
            }
        };

        append_path_segment(&mut builder, segment);
    }

    builder.finish().map(Arc::new)
}

fn append_path_segment(builder: &mut tiny_skia_path::PathBuilder, segment: svgtypes::SimplePathSegment) {
    match segment {
            svgtypes::SimplePathSegment::MoveTo { x, y } => {
                builder.move_to(x as f32, y as f32);
            }
            svgtypes::SimplePathSegment::LineTo { x, y } => {
                builder.line_to(x as f32, y as f32);
            }
            svgtypes::SimplePathSegment::Quadratic { x1, y1, x, y } => {
                builder.quad_to(x1 as f32, y1 as f32, x as f32, y as f32);
            }
            svgtypes::SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                builder.cubic_to(
                    x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32,
                );
            }
            svgtypes::SimplePathSegment::ClosePath => {
                builder.close();
            }
    }
}

fn convert_rect(node: SvgNode, state: &converter::State) -> Option<Arc<Path>> {
    let (width, height) = rect_size(node, state)?;
    let x = node.convert_user_length(AId::X, state, Length::zero());
    let y = node.convert_user_length(AId::Y, state, Length::zero());

    let (mut rx, mut ry) = resolve_rx_ry(node, state);

    // Clamp rx/ry to the half of the width/height.
    //
    // Should be done only after resolving.
    if rx > width / 2.0 {
        rx = width / 2.0;
    }
    if ry > height / 2.0 {
        ry = height / 2.0;
    }

    // Conversion according to https://www.w3.org/TR/SVG11/shapes.html#RectElement
    let path = rect_path(x, y, width, height, rx, ry)?;
    Some(Arc::new(path))
}

fn rect_size(node: SvgNode, state: &converter::State) -> Option<(f32, f32)> {
    // 'width' and 'height' attributes must be positive and non-zero.
    let width = node.convert_user_length(AId::Width, state, Length::zero());
    let height = node.convert_user_length(AId::Height, state, Length::zero());
    if !width.is_valid_length() {
        log::warn!(
            "Rect '{}' has an invalid 'width' value. Skipped.",
            node.element_id()
        );
        return None;
    }
    if !height.is_valid_length() {
        log::warn!(
            "Rect '{}' has an invalid 'height' value. Skipped.",
            node.element_id()
        );
        return None;
    }

    Some((width, height))
}

fn rect_path(x: f32, y: f32, width: f32, height: f32, rx: f32, ry: f32) -> Option<Path> {
    let path = if rx.approx_eq_ulps(&0.0, 4) {
        tiny_skia_path::PathBuilder::from_rect(Rect::from_xywh(x, y, width, height)?)
    } else {
        let mut builder = tiny_skia_path::PathBuilder::new();
        builder.move_to(x + rx, y);

        builder.line_to(x + width - rx, y);
        builder.arc_to(rx, ry, 0.0, false, true, x + width, y + ry);

        builder.line_to(x + width, y + height - ry);
        builder.arc_to(rx, ry, 0.0, false, true, x + width - rx, y + height);

        builder.line_to(x + rx, y + height);
        builder.arc_to(rx, ry, 0.0, false, true, x, y + height - ry);

        builder.line_to(x, y + ry);
        builder.arc_to(rx, ry, 0.0, false, true, x + rx, y);

        builder.close();

        builder.finish()?
    };

    Some(path)
}

fn resolve_rx_ry(node: SvgNode, state: &converter::State) -> (f32, f32) {
    let mut rx_opt = node.attribute::<Length>(AId::Rx);
    let mut ry_opt = node.attribute::<Length>(AId::Ry);

    // Remove negative values first.
    if let Some(v) = rx_opt {
        if v.number.is_sign_negative() {
            rx_opt = None;
        }
    }
    if let Some(v) = ry_opt {
        if v.number.is_sign_negative() {
            ry_opt = None;
        }
    }

    resolve_rect_radii(node, state, rx_opt, ry_opt)
}

fn resolve_rect_radii(node: SvgNode, state: &converter::State, rx_opt: Option<Length>, ry_opt: Option<Length>) -> (f32, f32) {
    match (rx_opt, ry_opt) {
        (None, None) => (0.0, 0.0),
        (Some(rx), None) => {
            let rx = units::convert_user_length(rx, node, AId::Rx, state);
            (rx, rx)
        }
        (None, Some(ry)) => {
            let ry = units::convert_user_length(ry, node, AId::Ry, state);
            (ry, ry)
        }
        (Some(rx), Some(ry)) => {
            let rx = units::convert_user_length(rx, node, AId::Rx, state);
            let ry = units::convert_user_length(ry, node, AId::Ry, state);
            (rx, ry)
        }
    }
}

fn convert_line(node: SvgNode, state: &converter::State) -> Option<Arc<Path>> {
    let x1 = node.convert_user_length(AId::X1, state, Length::zero());
    let y1 = node.convert_user_length(AId::Y1, state, Length::zero());
    let x2 = node.convert_user_length(AId::X2, state, Length::zero());
    let y2 = node.convert_user_length(AId::Y2, state, Length::zero());

    let mut builder = tiny_skia_path::PathBuilder::new();
    builder.move_to(x1, y1);
    builder.line_to(x2, y2);
    builder.finish().map(Arc::new)
}

fn convert_polyline(node: SvgNode) -> Option<Arc<Path>> {
    let builder = points_to_path(node, "Polyline")?;
    builder.finish().map(Arc::new)
}

fn convert_polygon(node: SvgNode) -> Option<Arc<Path>> {
    let mut builder = points_to_path(node, "Polygon")?;
    builder.close();
    builder.finish().map(Arc::new)
}

fn points_to_path(node: SvgNode, eid: &str) -> Option<tiny_skia_path::PathBuilder> {
    let mut builder = tiny_skia_path::PathBuilder::new();
    match node.attribute::<&str>(AId::Points) {
        Some(text) => append_points(&mut builder, text),
        _ => {
            log::warn!(
                "{} '{}' has an invalid 'points' value. Skipped.",
                eid,
                node.element_id()
            );
            return None;
        }
    };

    // 'polyline' and 'polygon' elements must contain at least 2 points.
    if builder.len() < 2 {
        log::warn!(
            "{} '{}' has less than 2 points. Skipped.",
            eid,
            node.element_id()
        );
        return None;
    }

    Some(builder)
}

fn append_points(builder: &mut tiny_skia_path::PathBuilder, text: &str) {
    for (x, y) in svgtypes::PointsParser::from(text) {
        if builder.is_empty() {
            builder.move_to(x as f32, y as f32);
        } else {
            builder.line_to(x as f32, y as f32);
        }
    }
}

fn convert_circle(node: SvgNode, state: &converter::State) -> Option<Arc<Path>> {
    let cx = node.convert_user_length(AId::Cx, state, Length::zero());
    let cy = node.convert_user_length(AId::Cy, state, Length::zero());
    let r = node.convert_user_length(AId::R, state, Length::zero());

    if !r.is_valid_length() {
        log::warn!(
            "Circle '{}' has an invalid 'r' value. Skipped.",
            node.element_id()
        );
        return None;
    }

    ellipse_to_path(cx, cy, r, r)
}

fn convert_ellipse(node: SvgNode, state: &converter::State) -> Option<Arc<Path>> {
    let cx = node.convert_user_length(AId::Cx, state, Length::zero());
    let cy = node.convert_user_length(AId::Cy, state, Length::zero());
    let (rx, ry) = resolve_rx_ry(node, state);

    if !rx.is_valid_length() {
        log::warn!(
            "Ellipse '{}' has an invalid 'rx' value. Skipped.",
            node.element_id()
        );
        return None;
    }

    if !ry.is_valid_length() {
        log::warn!(
            "Ellipse '{}' has an invalid 'ry' value. Skipped.",
            node.element_id()
        );
        return None;
    }

    ellipse_to_path(cx, cy, rx, ry)
}

fn ellipse_to_path(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Arc<Path>> {
    let mut builder = tiny_skia_path::PathBuilder::new();
    builder.move_to(cx + rx, cy);
    builder.arc_to(rx, ry, 0.0, false, true, cx, cy + ry);
    builder.arc_to(rx, ry, 0.0, false, true, cx - rx, cy);
    builder.arc_to(rx, ry, 0.0, false, true, cx, cy - ry);
    builder.arc_to(rx, ry, 0.0, false, true, cx + rx, cy);
    builder.close();
    builder.finish().map(Arc::new)
}

trait PathBuilderExt {
    fn arc_to(
        &mut self,
        rx: f32,
        ry: f32,
        x_axis_rotation: f32,
        large_arc: bool,
        sweep: bool,
        x: f32,
        y: f32,
    );
}

impl PathBuilderExt for tiny_skia_path::PathBuilder {
    fn arc_to(
        &mut self,
        rx: f32,
        ry: f32,
        x_axis_rotation: f32,
        large_arc: bool,
        sweep: bool,
        x: f32,
        y: f32,
    ) {
        let prev = match self.last_point() {
            Some(v) => v,
            None => return,
        };

        let svg_arc = kurbo::SvgArc {
            from: kurbo::Point::new(prev.x as f64, prev.y as f64),
            to: kurbo::Point::new(x as f64, y as f64),
            radii: kurbo::Vec2::new(rx as f64, ry as f64),
            x_rotation: (x_axis_rotation as f64).to_radians(),
            large_arc,
            sweep,
        };

        append_svg_arc(self, &svg_arc, x, y);
    }
}

fn append_svg_arc(builder: &mut tiny_skia_path::PathBuilder, svg_arc: &kurbo::SvgArc, x: f32, y: f32) {
        match kurbo::Arc::from_svg_arc(svg_arc) {
            Some(arc) => {
                arc.to_cubic_beziers(0.1, |p1, p2, p| {
                    builder.cubic_to(
                        p1.x as f32,
                        p1.y as f32,
                        p2.x as f32,
                        p2.y as f32,
                        p.x as f32,
                        p.y as f32,
                    );
                });
            }
            None => {
                builder.line_to(x, y);
            }
        }
}

#[cfg(test)]
mod tests {
    use super::PathBuilderExt;

    #[test]
    fn arc_to_on_empty_path_is_ignored_and_arc_after_move_to_builds_geometry()
    -> Result<(), String> {
        let mut empty_builder = tiny_skia_path::PathBuilder::new();
        empty_builder.arc_to(10.0, 10.0, 0.0, false, true, 20.0, 20.0);
        assert!(empty_builder.last_point().is_none());
        assert!(empty_builder.finish().is_none());

        let mut builder = tiny_skia_path::PathBuilder::new();
        builder.move_to(10.0, 10.0);
        builder.arc_to(10.0, 10.0, 0.0, false, true, 20.0, 20.0);

        let path = builder.finish().ok_or("arc after MoveTo did not build a path")?;
        assert!(path.len() > 1, "arc after MoveTo should add geometry");
        Ok(())
    }
}
