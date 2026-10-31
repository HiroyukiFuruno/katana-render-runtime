use super::GlyphPainter;
use skrifa::{
    FontRef,
    color::{Brush, ColorPainter, ColorStop, CompositeMode, Extend, Transform},
    instance::LocationRef,
    raw::types::{BoundingBox, Point},
};
use svgtypes::Color;
use xmlwriter::{Options, XmlWriter};

const FONT_BYTES: &[u8] =
    include_bytes!("../../../tests/fixtures/svg-engine/colr-public-fonts/colr_1.ttf");
const EXPECTED_INTERSECTION: &str = "M 5 3 L 10 3 L 10 8 L 5 8 Z";
const SWEEP_END_ANGLE_DEGREES: f32 = 90.0;
const FOREGROUND_COLOR: Color = Color {
    red: 17,
    green: 34,
    blue: 51,
    alpha: 255,
};
const PARENT_TRANSFORM: Transform = Transform {
    xx: 2.0,
    yx: 0.0,
    xy: 0.0,
    yy: 3.0,
    dx: 4.0,
    dy: 5.0,
};
const NESTED_TRANSFORM: Transform = Transform {
    xx: 1.0,
    yx: 0.0,
    xy: 0.0,
    yy: 1.0,
    dx: 7.0,
    dy: 8.0,
};

fn font() -> Result<FontRef<'static>, String> {
    FontRef::new(FONT_BYTES).map_err(|error| format!("COLR fixture font is invalid: {error:?}"))
}

fn writer() -> XmlWriter {
    let mut writer = XmlWriter::new(Options::default());
    writer.start_element("svg");
    writer
}

fn painter<'a>(
    font: &'a FontRef<'static>,
    svg: &'a mut XmlWriter,
    path_buf: &'a mut String,
) -> GlyphPainter<'a> {
    GlyphPainter {
        font,
        location: LocationRef::default(),
        svg,
        path_buf,
        gradient_index: 1,
        clip_path_index: 1,
        foreground_color: FOREGROUND_COLOR,
        transform: Transform::default(),
        outline_transform: Transform::default(),
        transforms_stack: Vec::new(),
        clip_stack: Vec::new(),
    }
}

fn box_bounds(x_min: f32, y_min: f32, x_max: f32, y_max: f32) -> BoundingBox<f32> {
    BoundingBox {
        x_min,
        y_min,
        x_max,
        y_max,
    }
}

fn solid_brush() -> Brush<'static> {
    Brush::Solid {
        palette_index: u16::MAX,
        alpha: 1.0,
    }
}

#[test]
fn intersecting_clip_boxes_fill_their_exact_intersection() -> Result<(), String> {
    let font = font()?;
    let mut svg = writer();
    let mut path = String::new();
    {
        let mut painter = painter(&font, &mut svg, &mut path);
        painter.push_clip_box(box_bounds(0.0, 0.0, 10.0, 10.0));
        painter.push_clip_box(box_bounds(5.0, 3.0, 15.0, 8.0));
        painter.fill(solid_brush());
        if painter.clip_stack.len() != 2 {
            return Err("both active clip boxes must remain stacked".to_owned());
        }
        painter.pop_clip();
        painter.pop_clip();
        if !painter.clip_stack.is_empty() {
            return Err("popping both clip boxes must restore an empty stack".to_owned());
        }
    }
    let output = finish(svg);
    if !output.contains(EXPECTED_INTERSECTION) || !output.contains("fill=\"rgb(17, 34, 51)\"") {
        return Err(format!(
            "clip intersection or foreground color missing: {output}"
        ));
    }
    Ok(())
}

fn finish(mut svg: XmlWriter) -> String {
    svg.end_element();
    svg.end_document()
}

#[test]
fn nonintersecting_clip_boxes_do_not_emit_a_fill() -> Result<(), String> {
    let font = font()?;
    let mut svg = writer();
    let mut path = String::new();
    {
        let mut painter = painter(&font, &mut svg, &mut path);
        painter.push_clip_box(box_bounds(0.0, 0.0, 2.0, 2.0));
        painter.push_clip_box(box_bounds(3.0, 3.0, 5.0, 5.0));
        painter.fill(solid_brush());
        painter.pop_clip();
        painter.pop_clip();
    }
    let output = finish(svg);
    if output.contains("fill=\"rgb(17, 34, 51)\"") {
        return Err("disjoint clip boxes must suppress fill output".to_owned());
    }
    Ok(())
}

#[test]
fn inverted_clip_box_suppresses_fill_and_can_be_popped() -> Result<(), String> {
    let font = font()?;
    let mut svg = writer();
    let mut path = String::new();
    {
        let mut painter = painter(&font, &mut svg, &mut path);
        painter.push_clip_box(box_bounds(4.0, 1.0, 3.0, 7.0));
        painter.fill(solid_brush());
        if painter.clip_stack.len() != 1 || painter.clip_stack[0].is_some() {
            return Err("inverted clip box must be retained as an empty clip".to_owned());
        }
        painter.pop_clip();
        if !painter.clip_stack.is_empty() {
            return Err("popping an invalid clip must restore the stack".to_owned());
        }
    }
    let output = finish(svg);
    if output.contains("fill=\"rgb(17, 34, 51)\"") {
        return Err("invalid clip box must suppress fill output".to_owned());
    }
    Ok(())
}

#[test]
fn zero_width_clip_box_retains_zero_area_bounds() -> Result<(), String> {
    let font = font()?;
    let mut svg = writer();
    let mut path = String::new();
    let mut painter = painter(&font, &mut svg, &mut path);
    painter.push_clip_box(box_bounds(4.0, 1.0, 4.0, 7.0));
    let Some(bounds) = painter.clip_stack.last().copied().flatten() else {
        return Err("zero-width clip box must remain a valid zero-area rectangle".to_owned());
    };
    if (bounds.left(), bounds.top(), bounds.right(), bounds.bottom()) != (4.0, 1.0, 4.0, 7.0) {
        return Err(format!("zero-width clip bounds changed: {bounds:?}"));
    }
    painter.pop_clip();
    if !painter.clip_stack.is_empty() {
        return Err("popping a zero-width clip must restore an empty stack".to_owned());
    }
    Ok(())
}

#[test]
fn transform_stack_restores_each_parent_matrix() -> Result<(), String> {
    let font = font()?;
    let mut svg = writer();
    let mut path = String::new();
    let mut painter = painter(&font, &mut svg, &mut path);
    painter.push_transform(PARENT_TRANSFORM);
    painter.push_transform(NESTED_TRANSFORM);
    painter.pop_transform();
    if painter.transform != PARENT_TRANSFORM {
        return Err("popping nested transform must restore its parent".to_owned());
    }
    painter.pop_transform();
    if painter.transform != Transform::default() || !painter.transforms_stack.is_empty() {
        return Err("popping parent transform must restore the initial state".to_owned());
    }
    Ok(())
}

fn paint_unsupported_sweep(painter: &mut GlyphPainter<'_>) {
    let stops = [ColorStop {
        offset: 0.0,
        palette_index: u16::MAX,
        alpha: 1.0,
    }];
    painter.fill(Brush::SweepGradient {
        c0: Point { x: 0.0, y: 0.0 },
        start_angle: 0.0,
        end_angle: SWEEP_END_ANGLE_DEGREES,
        color_stops: &stops,
        extend: Extend::Pad,
    });
}

#[test]
fn unsupported_brush_is_noop_and_unsupported_composite_uses_normal() -> Result<(), String> {
    let font = font()?;
    let mut svg = writer();
    let mut path = String::new();
    {
        let mut painter = painter(&font, &mut svg, &mut path);
        painter.push_clip_box(box_bounds(0.0, 0.0, 10.0, 10.0));
        paint_unsupported_sweep(&mut painter);
        painter.pop_clip();
        painter.push_layer(CompositeMode::Clear);
        painter.pop_layer();
    }
    let output = finish(svg);
    if output.contains("fill=\"rgb(17, 34, 51)\"")
        || !output.contains("mix-blend-mode: normal; isolation: isolate")
    {
        return Err(format!(
            "unsupported brush/composite contract changed: {output}"
        ));
    }
    Ok(())
}

#[path = "svg_engine_colr_outline_model_tests.rs"]
mod outline_model_tests;
