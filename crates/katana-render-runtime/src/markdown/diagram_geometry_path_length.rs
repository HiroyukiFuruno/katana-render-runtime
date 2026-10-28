use kurbo::{BezPath, Shape};

const PATH_LENGTH_ACCURACY: f64 = 1e-6;

pub(super) fn svg_path_length_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut return_value: v8::ReturnValue<v8::Value>,
) {
    let path_data = args.get(0).to_rust_string_lossy(scope);
    let length = BezPath::from_svg(&path_data)
        .ok()
        .map(|path| path.perimeter(PATH_LENGTH_ACCURACY))
        .filter(|length| length.is_finite() && *length >= 0.0)
        .and_then(|length| {
            let rounded = f64::from(length as f32);
            rounded.is_finite().then_some(rounded)
        })
        .unwrap_or(0.0);
    return_value.set(v8::Number::new(scope, length).into());
}
