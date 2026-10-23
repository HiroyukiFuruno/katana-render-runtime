const CANVAS_MEASURE_SEGMENTS_ARG: i32 = 5;

use super::{FontMeasureRequest, font_measure_request, measure_font_text};

pub(super) fn measure_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut return_value: v8::ReturnValue<v8::Value>,
) {
    let Some(request) = font_measure_request(scope, &args) else {
        return_value.set(v8::undefined(scope).into());
        return;
    };
    let segments = if args.length() > CANVAS_MEASURE_SEGMENTS_ARG {
        measure_segments(scope, &args, &request.text)
    } else {
        Some(Vec::new())
    };
    let Some(segments) = segments else {
        return_value.set(v8::undefined(scope).into());
        return;
    };
    match measure_width(&request, &segments) {
        Some(width) => return_value.set(v8::Number::new(scope, width).into()),
        None => return_value.set(v8::undefined(scope).into()),
    }
}

fn measure_width(request: &FontMeasureRequest, segments: &[String]) -> Option<f64> {
    let width = if segments.is_empty() {
        measure_text(request, &request.text)
    } else {
        segments.iter().try_fold(0.0_f64, |total, segment| {
            Some(total + measure_text(request, segment)?)
        })
    }?;
    (width.is_finite() && width >= 0.0).then_some(width)
}

fn measure_text(request: &FontMeasureRequest, text: &str) -> Option<f64> {
    let width = measure_font_text(request, text).ok()?;
    let width = f64::from(width);
    (width.is_finite() && width >= 0.0).then_some(width)
}

fn measure_segments(
    scope: &mut v8::PinScope,
    args: &v8::FunctionCallbackArguments,
    text: &str,
) -> Option<Vec<String>> {
    let array = v8::Local::<v8::Array>::try_from(args.get(CANVAS_MEASURE_SEGMENTS_ARG)).ok()?;
    let length = usize::try_from(array.length()).ok()?;
    if length > text.chars().count() {
        return None;
    }
    let mut segments = Vec::with_capacity(length);
    for index in 0..array.length() {
        let value = v8::Local::<v8::String>::try_from(array.get_index(scope, index)?).ok()?;
        let segment = value.to_rust_string_lossy(scope);
        if segment.is_empty() {
            return None;
        }
        segments.push(segment);
    }
    (segments.iter().map(String::as_str).collect::<String>() == text).then_some(segments)
}
