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
    let svg_name = v8::String::new(scope, "__katanaMeasureSvgTextMetrics")
        .ok_or_else(|| "SVG font metric bridge name allocation failed".to_string())?;
    let svg_callback = v8::Function::new(scope, svg_font_measure_callback)
        .ok_or_else(|| "SVG font metric bridge callback allocation failed".to_string())?;
    global
        .set(scope, svg_name.into(), svg_callback.into())
        .ok_or_else(|| "SVG font metric bridge registration failed".to_string())?;
    Ok(())
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

struct FontMeasureRequest {
    text: String,
    family: String,
    size: f32,
    weight: u16,
    italic: bool,
}

fn font_measure_request(
    scope: &mut v8::PinScope,
    args: &v8::FunctionCallbackArguments,
) -> Option<FontMeasureRequest> {
    font_measure_request_with_size_policy(scope, args, false)
}

fn font_measure_request_with_size_policy(
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

fn font_measure_number(
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

#[cfg(test)]
mod boundary_tests {
    use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};

    #[test]
    fn real_svg_bridge_distinguishes_true_zero_from_positive_f64_underflow() -> Result<(), String> {
        use crate::markdown::svg_rasterize::SvgRasterizeOps;

        let zero = SvgRasterizeOps::measure_svg_text_metrics("DB", "Noto Sans", 0.0, 400, false)
            .map_err(|error| format!("true zero SVG request failed: {error:?}"))?;
        assert_eq!([zero.advance, zero.ascent, zero.descent], [0.0; 3]);
        let script = r#"(() => {
            const svg = globalThis.__katanaMeasureSvgTextMetrics;
            const css = globalThis.__katanaMeasureText;
            const normal = svg('DB', 'Noto Sans', 16, 400, false);
            const zero = svg('DB', 'Noto Sans', 0, 400, false);
            const tiny = 1e-100;
            let rejected = false;
            try { svg('DB', 'Noto Sans', tiny, 400, false); }
            catch (error) { rejected = error instanceof Error; }
            return [normal.advance > 0 && normal.ascent + normal.descent > 0,
                [zero.advance, zero.ascent, zero.descent].every(value => value === 0),
                tiny > 0 && Math.fround(tiny) === 0,
                rejected, css('DB', 'Noto Sans', tiny, 400, false) === undefined,
                css('DB', 'Noto Sans', 0, 400, false) === undefined].join(',');
        })()"#;
        let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
            "bridge-svg-zero-positive-underflow",
            script,
        )])?;
        assert_eq!(result, "true,true,true,true,true,true");
        Ok(())
    }

    #[test]
    fn finite_javascript_size_outside_f32_request_range_returns_undefined() -> Result<(), String> {
        let script = r#"(() => {
            const measure = globalThis.__katanaMeasureText;
            const ordinary = measure('WWWW', 'Noto Sans', 16, 400, false);
            const large = 1e100;
            const rejected = measure('WWWW', 'Noto Sans', large, 400, false);
            return [Number.isFinite(ordinary) && ordinary > 0,
                Number.isFinite(large) && !Number.isFinite(Math.fround(large)),
                rejected === undefined].join(',');
        })()"#;
        let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
            "bridge-f32-request-range",
            script,
        )])?;
        assert_eq!(result, "true,true,true");
        Ok(())
    }

    #[test]
    fn nul_text_with_empty_family_returns_parse_error_and_undefined() {
        use crate::markdown::svg_rasterize::{SvgRasterizeError, SvgRasterizeOps};

        let invalid = SvgRasterizeOps::measure_html_text("\0", "", 16.0, 400, false, 0.0, None);
        assert!(matches!(invalid, Err(SvgRasterizeError::ParseFailed(_))));
        assert!(matches!(
            SvgRasterizeOps::measure_html_text("A", "", 16.0, 400, false, 0.0, None),
            Ok(width) if width.is_finite() && width >= 0.0
        ));
        let script = r#"(() => {
            const measure = globalThis.__katanaMeasureText;
            return measure('\u0000', '', 16, 400, false) === undefined;
        })()"#;
        let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
            "bridge-nul-empty-family-measurement",
            script,
        )]);
        assert!(matches!(result.as_deref(), Ok("true")));
    }
}
