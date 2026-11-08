use std::str::FromStr;

use super::DiagramTryCatchScope;
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
    let name = v8::String::new(scope, "__katanaTransformSvgBox")
        .ok_or_else(|| "SVG transform bridge name allocation failed".to_string())?;
    let callback = v8::Function::new(scope, transform_box_callback)
        .ok_or_else(|| "SVG transform bridge callback allocation failed".to_string())?;
    if global.set(scope, name.into(), callback.into()) != Some(true) {
        return Err("SVG transform bridge registration failed".to_string());
    }
    let length_name = v8::String::new(scope, "__katanaSvgPathLength")
        .ok_or_else(|| "SVG path length bridge name allocation failed".to_string())?;
    let length_callback = v8::Function::new(scope, path_length::svg_path_length_callback)
        .ok_or_else(|| "SVG path length bridge callback allocation failed".to_string())?;
    if global.set(scope, length_name.into(), length_callback.into()) != Some(true) {
        return Err("SVG path length bridge registration failed".to_string());
    }
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
        Ok(bounds) => {
            let object = v8::Object::new(scope);
            if set_box_properties(scope, object, bounds).is_ok() {
                return_value.set(object.into());
            } else {
                throw_transform_error(scope, "SVG transformed bounds could not be returned");
            }
        }
        Err(message) => throw_transform_error(scope, &message),
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
    match Transform::from_str(transform) {
        Ok(transform) => TransformedBox::new(transform, bounds),
        Err(_) => TransformedBox::new(Transform::default(), bounds),
    }
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
    let name = v8::String::new(scope, property)
        .ok_or_else(|| "SVG transformed bounds property allocation failed".to_string())?;
    let value = v8::Number::new(scope, value);
    /* WHY: 結果辞書の値をprototype setterに奪わせず、書込み不能も検出する。 */
    if object.create_data_property(scope, name.into(), value.into()) != Some(true) {
        return Err("SVG transformed bounds property write failed".to_string());
    }
    Ok(())
}

fn throw_transform_error(scope: &mut v8::PinScope, message: &str) {
    if let Some(message) = v8::String::new(scope, message) {
        let error = v8::Exception::error(scope, message);
        scope.throw_exception(error);
    }
}

#[cfg(test)]
#[path = "diagram_geometry_bridge_tests.rs"]
mod tests;
