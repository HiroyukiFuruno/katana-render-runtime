use std::str::FromStr;

use super::diagram_bridge_registration::register_callback;
use super::{DiagramTryCatchScope, set_bridge_number_property, throw_bridge_error};
use svgtypes::Transform;
#[cfg(target_os = "macos")]
#[path = "diagram_geometry_path_bounds.rs"]
mod path_bounds;
#[path = "diagram_geometry_path_length.rs"]
mod path_length;
const TRANSFORM_ARG: i32 = 0;
const X_ARG: i32 = 1;
const Y_ARG: i32 = 2;
const WIDTH_ARG: i32 = 3;
const HEIGHT_ARG: i32 = 4;
const BOUNDS_COMPONENT_COUNT: usize = 4;
const BOUNDS_WIDTH_INDEX: usize = 2;
const BOUNDS_HEIGHT_INDEX: usize = 3;
pub(super) fn install(scope: &mut DiagramTryCatchScope<'_, '_, '_, '_>) -> Result<(), String> {
    let context = scope.get_current_context();
    let global = context.global(scope);
    let _ = register_callback(
        scope,
        global,
        "__katanaTransformSvgBox",
        transform_box_callback,
        "SVG transform",
    )?;
    let _ = register_callback(
        scope,
        global,
        "__katanaSvgPathLength",
        path_length::svg_path_length_callback,
        "SVG path length",
    )?;
    #[cfg(target_os = "macos")]
    path_bounds::install(scope, global)?;
    Ok(())
}

fn transform_box_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut return_value: v8::ReturnValue<v8::Value>,
) {
    let result = transform_box_request(scope, &args);
    match result {
        Ok(bounds) => return_bounds(
            scope,
            &mut return_value,
            bounds,
            "SVG transformed bounds could not be returned",
        ),
        Err(message) => throw_bridge_error(scope, &message),
    }
}

fn return_bounds(
    scope: &mut v8::PinScope,
    result: &mut v8::ReturnValue<v8::Value>,
    bounds: TransformedBox,
    error_message: &str,
) {
    let object = v8::Object::new(scope);
    if set_box_properties(scope, object, bounds).is_ok() {
        result.set(object.into());
    } else {
        throw_bridge_error(scope, error_message);
    }
}

fn transform_box_request(
    scope: &mut v8::PinScope,
    args: &v8::FunctionCallbackArguments,
) -> Result<TransformedBox, String> {
    let transform = args.get(TRANSFORM_ARG).to_rust_string_lossy(scope);
    let values = [X_ARG, Y_ARG, WIDTH_ARG, HEIGHT_ARG]
        .map(|index| args.get(index).number_value(scope).unwrap_or(f64::NAN));
    transformed_box(&transform, values)
}

fn transformed_box(
    transform: &str,
    bounds: [f64; BOUNDS_COMPONENT_COUNT],
) -> Result<TransformedBox, String> {
    /* WHY: SVGの無効なtransform属性はブラウザと同じ恒等変換にし、bbox入力の検査は維持する。 */
    let transform = match Transform::from_str(transform) {
        Ok(transform) => transform,
        Err(_) => Transform::default(),
    };
    TransformedBox::new(transform, bounds)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TransformedBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
impl TransformedBox {
    fn new(transform: Transform, values: [f64; BOUNDS_COMPONENT_COUNT]) -> Result<Self, String> {
        if !values.into_iter().all(f64::is_finite)
            || values[BOUNDS_WIDTH_INDEX] < 0.0
            || values[BOUNDS_HEIGHT_INDEX] < 0.0
        {
            return Err("Invalid SVG child bounds".to_string());
        }
        let (x, y, width, height) = transformed_bounds(transform, values);
        [x, y, width, height]
            .into_iter()
            .all(f64::is_finite)
            .then_some(Self {
                x,
                y,
                width,
                height,
            })
            .ok_or_else(|| "Non-finite SVG transformed bounds".to_string())
    }
}

fn transformed_bounds(
    transform: Transform,
    bounds: [f64; BOUNDS_COMPONENT_COUNT],
) -> (f64, f64, f64, f64) {
    let [x, y, width, height] = bounds;
    let points = [
        transform_point(transform, x, y),
        transform_point(transform, x + width, y),
        transform_point(transform, x, y + height),
        transform_point(transform, x + width, y + height),
    ];
    let min_x = points
        .iter()
        .map(|point| point.0)
        .fold(f64::INFINITY, f64::min);
    let min_y = points
        .iter()
        .map(|point| point.1)
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = points
        .iter()
        .map(|point| point.1)
        .fold(f64::NEG_INFINITY, f64::max);
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

fn transform_point(transform: Transform, x: f64, y: f64) -> (f64, f64) {
    (
        transform.a * x + transform.c * y + transform.e,
        transform.b * x + transform.d * y + transform.f,
    )
}

fn set_box_properties(
    scope: &mut v8::PinScope,
    object: v8::Local<v8::Object>,
    bounds: TransformedBox,
) -> Result<(), String> {
    set_number_property(scope, object, "x", bounds.x)?;
    set_number_property(scope, object, "y", bounds.y)?;
    set_number_property(scope, object, "width", bounds.width)?;
    set_number_property(scope, object, "height", bounds.height)
}

fn set_number_property(
    scope: &mut v8::PinScope,
    object: v8::Local<v8::Object>,
    property: &'static str,
    value: f64,
) -> Result<(), String> {
    set_bridge_number_property(
        scope,
        object,
        property,
        value,
        "SVG transformed bounds property allocation failed",
        "SVG transformed bounds property write failed",
    )
    .map_err(str::to_owned)
}

#[cfg(test)]
#[path = "diagram_geometry_bridge_tests.rs"]
mod tests;
