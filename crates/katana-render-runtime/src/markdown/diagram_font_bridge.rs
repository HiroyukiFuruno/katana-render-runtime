const FONT_MEASURE_TEXT_ARG: i32 = 0;
const FONT_MEASURE_FAMILY_ARG: i32 = 1;
const FONT_MEASURE_SIZE_ARG: i32 = 2;
const FONT_MEASURE_WEIGHT_ARG: i32 = 3;
const FONT_MEASURE_ITALIC_ARG: i32 = 4;
const DEFAULT_FONT_SIZE: f64 = 16.0;
const DEFAULT_FONT_WEIGHT: f64 = 400.0;
const MIN_FONT_WEIGHT: f64 = 1.0;
const MAX_FONT_WEIGHT: f64 = 1000.0;
const CSS_LAYOUT_SUBPIXEL_SCALE: f64 = 64.0;

type DiagramTryCatchScope<'pin, 'scope, 'object, 'isolate> =
    v8::PinnedRef<'pin, v8::TryCatch<'scope, 'object, v8::HandleScope<'isolate>>>;

pub(super) fn install(scope: &mut DiagramTryCatchScope<'_, '_, '_, '_>) -> Result<(), String> {
    let context = scope.get_current_context();
    let global = context.global(scope);
    let name = v8::String::new(scope, "__katanaMeasureText")
        .ok_or_else(|| "font measure bridge name allocation failed".to_string())?;
    let callback = v8::Function::new(scope, font_measure_callback)
        .ok_or_else(|| "font measure bridge callback allocation failed".to_string())?;
    global
        .set(scope, name.into(), callback.into())
        .ok_or_else(|| "font measure bridge registration failed".to_string())?;
    super::diagram_svg_font_bridge::install(scope)
}

fn font_measure_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut return_value: v8::ReturnValue<v8::Value>,
) {
    let Some(request) = font_measure_request(scope, &args) else {
        return_value.set(v8::undefined(scope).into());
        return;
    };
    let width = crate::markdown::svg_rasterize::SvgRasterizeOps::measure_html_text(
        &request.text,
        &request.family,
        request.size,
        request.weight,
        request.italic,
        0.0,
        None,
    );
    match width {
        Ok(width) if width.is_finite() && width >= 0.0 => {
            let css_layout_width =
                (f64::from(width) * CSS_LAYOUT_SUBPIXEL_SCALE).ceil() / CSS_LAYOUT_SUBPIXEL_SCALE;
            return_value.set(v8::Number::new(scope, css_layout_width).into());
        }
        _ => return_value.set(v8::undefined(scope).into()),
    }
}

pub(super) struct FontMeasureRequest {
    pub(super) text: String,
    pub(super) family: String,
    pub(super) size: f32,
    pub(super) weight: u16,
    pub(super) italic: bool,
}

fn font_measure_request(
    scope: &mut v8::PinScope,
    args: &v8::FunctionCallbackArguments,
) -> Option<FontMeasureRequest> {
    font_measure_request_with_size_policy(scope, args, false)
}

pub(super) fn font_measure_request_with_size_policy(
    scope: &mut v8::PinScope,
    args: &v8::FunctionCallbackArguments,
    allow_zero: bool,
) -> Option<FontMeasureRequest> {
    let original_size = font_measure_number(scope, args, FONT_MEASURE_SIZE_ARG, DEFAULT_FONT_SIZE)
        .filter(|value| *value > 0.0 || (allow_zero && *value == 0.0))?;
    let size = original_size as f32;
    /* WHY: 正のJSサイズが変換で0になっても、本来のサイズ0として受け付けない。 */
    if !size.is_finite() || (original_size > 0.0 && size == 0.0) {
        return None;
    }
    let weight = font_measure_number(scope, args, FONT_MEASURE_WEIGHT_ARG, DEFAULT_FONT_WEIGHT)
        .filter(|value| (MIN_FONT_WEIGHT..=MAX_FONT_WEIGHT).contains(value))
        .map(|value| value as u16)?;
    Some(FontMeasureRequest {
        text: args.get(FONT_MEASURE_TEXT_ARG).to_rust_string_lossy(scope),
        family: args
            .get(FONT_MEASURE_FAMILY_ARG)
            .to_rust_string_lossy(scope),
        size,
        weight,
        italic: args.get(FONT_MEASURE_ITALIC_ARG).boolean_value(scope),
    })
}

pub(super) fn font_measure_number(
    scope: &mut v8::PinScope,
    args: &v8::FunctionCallbackArguments,
    index: i32,
    default: f64,
) -> Option<f64> {
    let value = if args.length() > index {
        args.get(index).number_value(scope)?
    } else {
        default
    };
    value.is_finite().then_some(value)
}

#[cfg(test)]
#[path = "diagram_font_bridge_boundary_tests.rs"]
mod boundary_tests;

#[cfg(test)]
#[path = "diagram_font_bridge_numeric_boundary_tests.rs"]
mod numeric_boundary_tests;
#[cfg(test)]
#[path = "diagram_font_bridge_x_height_tests.rs"]
mod x_height_tests;
