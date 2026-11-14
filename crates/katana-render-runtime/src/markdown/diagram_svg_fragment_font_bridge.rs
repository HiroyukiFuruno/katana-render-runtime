use super::super::diagram_bridge_registration::register_callback;
use super::super::diagram_font_bridge::font_measure_request_with_known_text;
use super::{set_svg_metric_properties, throw_bridge_error};
use crate::markdown::svg_rasterize::SvgRasterizeOps;

const MAX_FRAGMENT_COUNT: u32 = 4096;

pub(super) fn install(
    scope: &mut v8::PinScope,
    global: v8::Local<v8::Object>,
) -> Result<(), String> {
    register_callback(
        scope,
        global,
        "__katanaMeasureSvgFragmentLayout",
        callback,
        "SVG fragment",
    )
    .map(|_| ())
}

fn callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut result: v8::ReturnValue<v8::Value>,
) {
    match measure(scope, &args) {
        Ok(Some(value)) => result.set(value.into()),
        Ok(None) => result.set(v8::null(scope).into()),
        Err(message) => throw_bridge_error(scope, message),
    }
}

fn measure<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    args: &v8::FunctionCallbackArguments,
) -> Result<Option<v8::Local<'a, v8::Object>>, &'static str> {
    let fragments = read_fragments(scope, args).ok_or("Invalid SVG fragment request")?;
    let request = font_measure_request_with_known_text(scope, args, false, String::new())
        .ok_or("Invalid SVG fragment request")?;
    let Some(fragments) = fragments else {
        return Ok(None);
    };
    let layout = SvgRasterizeOps::measure_svg_fragment_layout(
        &fragments,
        &request.family,
        request.size,
        request.weight,
        request.italic,
    )
    .map_err(|_| "SVG fragment layout measurement failed")?;
    let Some(layout) = layout else {
        return Ok(None);
    };
    let object = v8::Object::new(scope);
    set_svg_metric_properties(scope, object, layout.metrics)?;
    set_advances(scope, object, &layout.advances)?;
    Ok(Some(object))
}

fn read_fragments(
    scope: &mut v8::PinScope,
    args: &v8::FunctionCallbackArguments,
) -> Option<Option<Vec<String>>> {
    let array = v8::Local::<v8::Array>::try_from(args.get(0)).ok()?;
    let length = array.length();
    let mut fragments = (length <= MAX_FRAGMENT_COUNT).then(|| Vec::with_capacity(length as usize));
    for index in 0..length {
        let value = v8::Local::<v8::String>::try_from(array.get_index(scope, index)?).ok()?;
        if let Some(fragments) = &mut fragments {
            fragments.push(value.to_rust_string_lossy(scope));
        }
    }
    Some(fragments)
}

fn set_advances(
    scope: &mut v8::PinScope,
    object: v8::Local<v8::Object>,
    advances: &[f32],
) -> Result<(), &'static str> {
    let length = i32::try_from(advances.len()).map_err(|_| "SVG fragment result too large")?;
    let array = v8::Array::new(scope, length);
    for (index, value) in (0_u32..).zip(advances) {
        let number = v8::Number::new(scope, f64::from(*value));
        if array.set_index(scope, index, number.into()) != Some(true) {
            return Err("SVG fragment result allocation failed");
        }
    }
    let key = v8::String::new(scope, "advances").ok_or("SVG fragment key allocation failed")?;
    if object.create_data_property(scope, key.into(), array.into()) != Some(true) {
        return Err("SVG fragment result allocation failed");
    }
    Ok(())
}

#[cfg(test)]
#[path = "diagram_svg_fragment_font_bridge_tests.rs"]
mod tests;
