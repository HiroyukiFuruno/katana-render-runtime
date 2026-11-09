use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use super::{
    BaseGradient, Cache, Color, NonEmptyString, Opacity, Paint, PositiveF32, RadialGradient, Rect,
    SpreadMethod, Stop, StopOffset, Transform, Units, process_context_paint, process_paint,
};
use tiny_skia_path::Point;

const GRADIENT_ID: &str = "authored-radial";
const CENTER_X: f32 = 0.25;
const CENTER_Y: f32 = 0.5;
const RADIUS: f32 = 0.75;
const FOCAL_X: f32 = 0.375;
const FOCAL_Y: f32 = 0.625;
const FOCAL_RADIUS: f32 = 0.125;
const FIRST_OFFSET: f32 = 0.0;
const SECOND_OFFSET: f32 = 1.0;
const FIRST_OPACITY: f32 = 0.75;
const SECOND_OPACITY: f32 = 0.5;
const FIRST_RED: u8 = 13;
const FIRST_GREEN: u8 = 17;
const FIRST_BLUE: u8 = 19;
const SECOND_RED: u8 = 23;
const SECOND_GREEN: u8 = 29;
const SECOND_BLUE: u8 = 31;
const COORDINATE_EPSILON: f32 = 0.001;
const ORIGIN_COORDINATE: f32 = 0.0;
const UNIT_COORDINATE: f32 = 1.0;
const CONTEXT_X: f32 = 2.0;
const CONTEXT_Y: f32 = 3.0;
const PATH_X: f32 = 7.0;
const PATH_Y: f32 = 11.0;
const CONTEXT_DELTA_X: f32 = -5.0;
const CONTEXT_DELTA_Y: f32 = -8.0;
const BBOX_X: f32 = 10.0;
const BBOX_Y: f32 = 20.0;
const BBOX_WIDTH: f32 = 100.0;
const BBOX_HEIGHT: f32 = 50.0;
const TRANSFORM_PROBE_COUNT: usize = 3;

fn positive(value: f32) -> Result<PositiveF32, String> {
    PositiveF32::new(value).ok_or_else(|| "positive radial value rejected".to_owned())
}

fn stop(offset: f32, color: Color, opacity: f32) -> Result<Stop, String> {
    Ok(Stop {
        offset: StopOffset::new(offset).ok_or_else(|| "stop offset rejected".to_owned())?,
        color,
        opacity: Opacity::new(opacity).ok_or_else(|| "stop opacity rejected".to_owned())?,
    })
}

fn authored_stops() -> Result<Vec<Stop>, String> {
    Ok(vec![
        stop(
            FIRST_OFFSET,
            Color::new_rgb(FIRST_RED, FIRST_GREEN, FIRST_BLUE),
            FIRST_OPACITY,
        )?,
        stop(
            SECOND_OFFSET,
            Color::new_rgb(SECOND_RED, SECOND_GREEN, SECOND_BLUE),
            SECOND_OPACITY,
        )?,
    ])
}

fn authored_gradient(units: Units) -> Result<Arc<RadialGradient>, String> {
    Ok(Arc::new(RadialGradient {
        base: BaseGradient {
            id: NonEmptyString::new(GRADIENT_ID.to_owned())
                .ok_or_else(|| "authored gradient ID is empty".to_owned())?,
            units,
            transform: Transform::identity(),
            spread_method: SpreadMethod::Pad,
            stops: authored_stops()?,
        },
        cx: CENTER_X,
        cy: CENTER_Y,
        r: positive(RADIUS)?,
        fx: FOCAL_X,
        fy: FOCAL_Y,
        fr: positive(FOCAL_RADIUS)?,
    }))
}

fn cache() -> Cache {
    let font_database = Arc::new(fontdb::Database::new());
    Cache::new(font_database.clone(), Rc::new(RefCell::new(font_database)))
}

fn radial_gradient(paint: &Paint) -> Result<&Arc<RadialGradient>, String> {
    match paint {
        Paint::RadialGradient(gradient) => Ok(gradient),
        _ => Err("paint is no longer a radial gradient".to_owned()),
    }
}

fn near(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() <= COORDINATE_EPSILON
}

fn mapped_point(transform: Transform, x: f32, y: f32) -> Point {
    let mut point = Point::from_xy(x, y);
    transform.map_point(&mut point);
    point
}

fn assert_transform_points(
    transform: Transform,
    expected: [Point; TRANSFORM_PROBE_COUNT],
) -> Result<(), String> {
    let actual = [
        mapped_point(transform, ORIGIN_COORDINATE, ORIGIN_COORDINATE),
        mapped_point(transform, UNIT_COORDINATE, ORIGIN_COORDINATE),
        mapped_point(transform, ORIGIN_COORDINATE, UNIT_COORDINATE),
    ];
    for (point, expected) in actual.iter().zip(expected) {
        if !near(point.x, expected.x) || !near(point.y, expected.y) {
            return Err("radial paint transform differs from authored geometry".to_owned());
        }
    }
    Ok(())
}

fn assert_authored_radial_values(gradient: &RadialGradient) -> Result<(), String> {
    if !near(gradient.cx(), CENTER_X)
        || !near(gradient.cy(), CENTER_Y)
        || !near(gradient.r().get(), RADIUS)
        || !near(gradient.fx(), FOCAL_X)
        || !near(gradient.fy(), FOCAL_Y)
        || !near(gradient.fr().get(), FOCAL_RADIUS)
        || gradient.stops().len() != 2
        || gradient.spread_method() != SpreadMethod::Pad
    {
        return Err("authored radial gradient parameters changed".to_owned());
    }

    let [first, second] = gradient.stops() else {
        return Err("authored radial stops are missing".to_owned());
    };
    if !near(first.offset().get(), FIRST_OFFSET)
        || first.color() != Color::new_rgb(FIRST_RED, FIRST_GREEN, FIRST_BLUE)
        || !near(first.opacity().get(), FIRST_OPACITY)
        || !near(second.offset().get(), SECOND_OFFSET)
        || second.color() != Color::new_rgb(SECOND_RED, SECOND_GREEN, SECOND_BLUE)
        || !near(second.opacity().get(), SECOND_OPACITY)
    {
        return Err("authored radial stop data changed".to_owned());
    }
    Ok(())
}

fn assert_identity_transform(transform: Transform) -> Result<(), String> {
    assert_transform_points(
        transform,
        [
            Point::from_xy(ORIGIN_COORDINATE, ORIGIN_COORDINATE),
            Point::from_xy(UNIT_COORDINATE, ORIGIN_COORDINATE),
            Point::from_xy(ORIGIN_COORDINATE, UNIT_COORDINATE),
        ],
    )
}

fn assert_source_gradient(gradient: &RadialGradient, units: Units) -> Result<(), String> {
    if gradient.id() != GRADIENT_ID || gradient.base.units != units {
        return Err("authored radial gradient identity or units changed".to_owned());
    }
    assert_authored_radial_values(gradient)?;
    assert_identity_transform(gradient.transform())
}

fn singular_skew_transform() -> Transform {
    Transform::from_row(
        UNIT_COORDINATE,
        UNIT_COORDINATE,
        UNIT_COORDINATE,
        UNIT_COORDINATE,
        ORIGIN_COORDINATE,
        ORIGIN_COORDINATE,
    )
}

fn assert_context_conversion(original: &Arc<RadialGradient>, paint: &Paint) -> Result<(), String> {
    let converted = radial_gradient(paint)?;
    if Arc::ptr_eq(original, converted) || converted.id() == GRADIENT_ID {
        return Err("context paint did not preserve the original Arc".to_owned());
    }
    assert_source_gradient(original, Units::UserSpaceOnUse)?;
    assert_authored_radial_values(converted)?;
    if converted.base.units != Units::UserSpaceOnUse {
        return Err("context paint changed radial gradient units".to_owned());
    }
    assert_transform_points(
        converted.transform(),
        [
            Point::from_xy(CONTEXT_DELTA_X, CONTEXT_DELTA_Y),
            Point::from_xy(CONTEXT_DELTA_X + UNIT_COORDINATE, CONTEXT_DELTA_Y),
            Point::from_xy(CONTEXT_DELTA_X, CONTEXT_DELTA_Y + UNIT_COORDINATE),
        ],
    )
}

#[test]
fn radial_context_paint_uses_inverse_translation_and_preserves_gradient_data() -> Result<(), String>
{
    let source = authored_gradient(Units::UserSpaceOnUse)?;
    let original = Arc::clone(&source);
    let mut paint = Paint::RadialGradient(source);
    let mut cache = cache();
    let context = Transform::from_translate(CONTEXT_X, CONTEXT_Y);
    let path = Transform::from_translate(PATH_X, PATH_Y);

    if process_context_paint(&mut paint, context, path, &mut cache).is_none() {
        return Err("invertible context transforms were rejected".to_owned());
    }
    assert_context_conversion(&original, &paint)
}

#[test]
fn singular_skew_context_returns_none_without_mutating_the_paint() -> Result<(), String> {
    let source = authored_gradient(Units::UserSpaceOnUse)?;
    let original = Arc::clone(&source);
    let mut paint = Paint::RadialGradient(source);
    let before = paint.clone();
    let mut cache = cache();
    let singular_context = singular_skew_transform();

    let result = process_context_paint(
        &mut paint,
        singular_context,
        Transform::identity(),
        &mut cache,
    );
    if result.is_some() {
        return Err("singular skew context transform was accepted".to_owned());
    }
    if paint != before {
        return Err("singular skew context mutated paint equality".to_owned());
    }
    if !Arc::ptr_eq(&original, radial_gradient(&paint)?) {
        return Err("singular skew context replaced the source Arc".to_owned());
    }
    assert_source_gradient(&original, Units::UserSpaceOnUse)?;
    Ok(())
}

#[path = "svg_engine_paint_server_bbox_tests.rs"]
mod bbox_tests;
