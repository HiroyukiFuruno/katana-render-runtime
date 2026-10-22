use super::DiagramTryCatchScope;
use super::diagram_font_bridge::{font_measure_number, font_measure_request_with_size_policy};

const FONT_X_HEIGHT_FAMILY_ARG: i32 = 0;
const FONT_X_HEIGHT_SIZE_ARG: i32 = 1;
const FONT_X_HEIGHT_WEIGHT_ARG: i32 = 2;
const FONT_X_HEIGHT_ITALIC_ARG: i32 = 3;
const DEFAULT_FONT_SIZE: f64 = 16.0;
const DEFAULT_FONT_WEIGHT: f64 = 400.0;
const MIN_FONT_WEIGHT: f64 = 1.0;
const MAX_FONT_WEIGHT: f64 = 1000.0;

pub(super) fn install(scope: &mut DiagramTryCatchScope<'_, '_, '_, '_>) -> Result<(), String> {
    let context = scope.get_current_context();
    let global = context.global(scope);
    let svg_name = v8::String::new(scope, "__katanaMeasureSvgTextMetrics")
        .ok_or_else(|| "SVG font metric bridge name allocation failed".to_string())?;
    let svg_callback = v8::Function::new(scope, svg_font_measure_callback)
        .ok_or_else(|| "SVG font metric bridge callback allocation failed".to_string())?;
    global
        .set(scope, svg_name.into(), svg_callback.into())
        .ok_or_else(|| "SVG font metric bridge registration failed".to_string())?;
    let x_height_name = v8::String::new(scope, "__katanaMeasureSvgFontXHeight")
        .ok_or_else(|| "SVG font x-height bridge name allocation failed".to_string())?;
    let x_height_callback = v8::Function::new(scope, svg_font_x_height_callback)
        .ok_or_else(|| "SVG font x-height bridge callback allocation failed".to_string())?;
    global
        .set(scope, x_height_name.into(), x_height_callback.into())
        .ok_or_else(|| "SVG font x-height bridge registration failed".to_string())?;
    Ok(())
}

fn svg_font_measure_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut return_value: v8::ReturnValue<v8::Value>,
) {
    let metrics = match svg_font_metrics(scope, &args) {
        Ok(metrics) => metrics,
        Err(message) => {
            throw_svg_metric_error(scope, message);
            return;
        }
    };
    let object = v8::Object::new(scope);
    if let Err(message) = set_svg_metric_properties(scope, object, metrics) {
        throw_svg_metric_error(scope, message);
        return;
    }
    return_value.set(object.into());
}

fn svg_font_x_height_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut return_value: v8::ReturnValue<v8::Value>,
) {
    let Some(request) = svg_font_x_height_request(scope, &args) else {
        throw_svg_metric_error(scope, "Invalid SVG font x-height request");
        return;
    };
    let measured = crate::markdown::svg_rasterize::SvgRasterizeOps::measure_svg_font_x_height(
        &request.family,
        request.size,
        request.weight,
        request.italic,
    );
    match measured {
        Ok(Some(x_height)) => return_value.set(v8::Number::new(scope, f64::from(x_height)).into()),
        Ok(None) => return_value.set(v8::undefined(scope).into()),
        Err(_) => throw_svg_metric_error(scope, "SVG font x-height measurement failed"),
    }
}

struct SvgFontXHeightRequest {
    family: String,
    size: f32,
    weight: u16,
    italic: bool,
}

fn svg_font_x_height_request(
    scope: &mut v8::PinScope,
    args: &v8::FunctionCallbackArguments,
) -> Option<SvgFontXHeightRequest> {
    let original_size = font_measure_number(scope, args, FONT_X_HEIGHT_SIZE_ARG, DEFAULT_FONT_SIZE)
        .filter(|value| *value > 0.0)?;
    let size = original_size as f32;
    if !size.is_finite() || size == 0.0 {
        return None;
    }
    let weight = font_measure_number(scope, args, FONT_X_HEIGHT_WEIGHT_ARG, DEFAULT_FONT_WEIGHT)
        .filter(|value| (MIN_FONT_WEIGHT..=MAX_FONT_WEIGHT).contains(value))?
        as u16;
    Some(SvgFontXHeightRequest {
        family: args
            .get(FONT_X_HEIGHT_FAMILY_ARG)
            .to_rust_string_lossy(scope),
        size,
        weight,
        italic: args.get(FONT_X_HEIGHT_ITALIC_ARG).boolean_value(scope),
    })
}

fn set_svg_metric_properties(
    scope: &mut v8::PinScope,
    object: v8::Local<v8::Object>,
    metrics: crate::markdown::svg_rasterize::SvgTextRunMetrics,
) -> Result<(), &'static str> {
    let properties = [
        ("advance", f64::from(metrics.advance)),
        ("ascent", f64::from(metrics.ascent)),
        ("descent", f64::from(metrics.descent)),
    ]
    .into_iter()
    .chain(metrics.ink_bounds.into_iter().flat_map(|bounds| {
        ["inkLeft", "inkTop", "inkRight", "inkBottom"]
            .into_iter()
            .zip(bounds)
    }));
    for (name, value) in properties {
        set_svg_metric_property(scope, object, name, value)?;
    }
    if let Some(ascent) = metrics.layout_ascent {
        set_svg_metric_property(scope, object, "layoutAscent", f64::from(ascent))?;
    }
    Ok(())
}

fn svg_font_metrics(
    scope: &mut v8::PinScope,
    args: &v8::FunctionCallbackArguments,
) -> Result<crate::markdown::svg_rasterize::SvgTextRunMetrics, &'static str> {
    let request = font_measure_request_with_size_policy(scope, args, true)
        .ok_or("Invalid SVG font metric request")?;
    crate::markdown::svg_rasterize::SvgRasterizeOps::measure_svg_text_metrics(
        &request.text,
        &request.family,
        request.size,
        request.weight,
        request.italic,
    )
    .map_err(|_| "SVG font metric measurement failed")
}

fn set_svg_metric_property(
    scope: &mut v8::PinScope,
    object: v8::Local<v8::Object>,
    name: &str,
    value: f64,
) -> Result<(), &'static str> {
    let key = v8::String::new(scope, name).ok_or("SVG font metric key allocation failed")?;
    let value = v8::Number::new(scope, value);
    if object.set(scope, key.into(), value.into()) != Some(true) {
        return Err("SVG font metric result allocation failed");
    }
    Ok(())
}

fn throw_svg_metric_error(scope: &mut v8::PinScope, message: &str) {
    if let Some(message) = v8::String::new(scope, message) {
        let error = v8::Exception::error(scope, message);
        scope.throw_exception(error);
    }
}
