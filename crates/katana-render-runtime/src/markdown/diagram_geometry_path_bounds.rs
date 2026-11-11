use super::{TransformedBox, register_callback, return_bounds, throw_bridge_error};

pub(super) fn install(
    scope: &mut v8::PinScope,
    global: v8::Local<v8::Object>,
) -> Result<(), String> {
    register_callback(
        scope,
        global,
        "__katanaSvgPathBounds",
        path_bounds_callback,
        "SVG path bounds",
    )
    .map(|_| ())
}

fn path_bounds_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut result: v8::ReturnValue<v8::Value>,
) {
    let source = args.get(0).to_rust_string_lossy(scope);
    match path_bounds(&source) {
        Ok(Some(bounds)) => return_bounds(
            scope,
            &mut result,
            bounds,
            "SVG path bounds could not be returned",
        ),
        Ok(None) => result.set(v8::null(scope).into()),
        Err(message) => throw_bridge_error(scope, message),
    }
}

fn path_bounds(source: &str) -> Result<Option<TransformedBox>, &'static str> {
    if source.is_empty() || source.contains('\0') {
        return Ok(None);
    }
    let Some(path) = skia_safe::Path::from_svg(source) else {
        return Ok(None);
    };
    let bounds = path.compute_tight_bounds();
    if !bounds.is_finite() || !bounds.width().is_finite() || !bounds.height().is_finite() {
        return Err("SVG path bounds are non-finite");
    }
    Ok(Some(TransformedBox {
        x: f64::from(bounds.left),
        y: f64::from(bounds.top),
        width: f64::from(bounds.width()),
        height: f64::from(bounds.height()),
    }))
}
