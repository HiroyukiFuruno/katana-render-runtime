use super::{TransformedBox, set_box_properties, throw_transform_error};

pub(super) fn install(
    scope: &mut v8::PinScope,
    global: v8::Local<v8::Object>,
) -> Result<(), String> {
    let name = v8::String::new(scope, "__katanaSvgPathBounds")
        .ok_or_else(|| "SVG path bounds bridge name allocation failed".to_string())?;
    let callback = v8::Function::new(scope, path_bounds_callback)
        .ok_or_else(|| "SVG path bounds bridge callback allocation failed".to_string())?;
    if global.set(scope, name.into(), callback.into()) != Some(true) {
        return Err("SVG path bounds bridge registration failed".to_string());
    }
    Ok(())
}

fn path_bounds_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut result: v8::ReturnValue<v8::Value>,
) {
    let source = args.get(0).to_rust_string_lossy(scope);
    match path_bounds(&source) {
        Ok(Some(bounds)) => {
            let object = v8::Object::new(scope);
            if set_box_properties(scope, object, bounds).is_ok() {
                result.set(object.into());
            } else {
                throw_transform_error(scope, "SVG path bounds could not be returned");
            }
        }
        Ok(None) => result.set(v8::null(scope).into()),
        Err(message) => throw_transform_error(scope, message),
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
