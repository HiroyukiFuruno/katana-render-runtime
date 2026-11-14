use super::diagram_bridge_registration::install_callback;
use super::diagram_font_bridge::{font_measure_number, font_measure_request_with_size_policy};
use super::{
    DiagramTryCatchScope, completed_bridge_object, set_bridge_number_property, throw_bridge_error,
};

#[path = "diagram_svg_fragment_font_bridge.rs"]
mod fragment;

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
    install_callback(
        scope,
        global,
        "__katanaMeasureSvgTextMetrics",
        svg_font_measure_callback,
        "SVG font metric",
    )?;
    install_callback(
        scope,
        global,
        "__katanaMeasureSvgFontXHeight",
        svg_font_x_height_callback,
        "SVG font x-height",
    )?;
    fragment::install(scope, global)?;
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
            throw_bridge_error(scope, message);
            return;
        }
    };
    let object = v8::Object::new(scope);
    let written = set_svg_metric_properties(scope, object, metrics);
    if let Some(object) = completed_bridge_object(scope, object, written) {
        return_value.set(object.into());
    }
}

fn svg_font_x_height_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut return_value: v8::ReturnValue<v8::Value>,
) {
    let Some(request) = svg_font_x_height_request(scope, &args) else {
        throw_bridge_error(scope, "Invalid SVG font x-height request");
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
        Err(_) => throw_bridge_error(scope, "SVG font x-height measurement failed"),
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
    set_bridge_number_property(
        scope,
        object,
        name,
        value,
        "SVG font metric key allocation failed",
        "SVG font metric result allocation failed",
    )
}

#[cfg(test)]
#[path = "diagram_svg_font_bridge_tests.rs"]
mod tests;
