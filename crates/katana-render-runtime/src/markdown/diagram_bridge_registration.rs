type RegisteredCallback<'scope> = (
    v8::Local<'scope, v8::String>,
    v8::Local<'scope, v8::Function>,
);

pub(super) fn register_callback<'scope>(
    scope: &mut v8::PinScope<'scope, '_>,
    global: v8::Local<v8::Object>,
    name: &str,
    callback: impl v8::MapFnTo<v8::FunctionCallback>,
    label: &str,
) -> Result<RegisteredCallback<'scope>, String> {
    let name = require_bridge_handle(v8::String::new(scope, name), label, "name")?;
    let callback = require_bridge_handle(v8::Function::new(scope, callback), label, "callback")?;
    if global.set(scope, name.into(), callback.into()) != Some(true) {
        return Err(registration_error(label));
    }
    Ok((name, callback))
}

pub(super) fn install_callback(
    scope: &mut v8::PinScope,
    global: v8::Local<v8::Object>,
    name: &str,
    callback: impl v8::MapFnTo<v8::FunctionCallback>,
    label: &str,
) -> Result<(), String> {
    let (name, callback) = register_callback(scope, global, name, callback, label)?;
    /* WHY: V8のset成功だけでは実際にcallbackが登録されたとは限らない。 */
    if !global
        .get(scope, name.into())
        .is_some_and(|value| value.strict_equals(callback.into()))
    {
        return Err(registration_error(label));
    }
    Ok(())
}

fn registration_error(label: &str) -> String {
    format!("{label} bridge registration failed")
}

fn require_bridge_handle<T>(handle: Option<T>, label: &str, kind: &str) -> Result<T, String> {
    handle.ok_or_else(|| format!("{label} bridge {kind} allocation failed"))
}

#[cfg(test)]
mod tests {
    use super::require_bridge_handle;

    #[test]
    fn missing_bridge_handles_preserve_the_failed_operation() {
        for (kind, expected) in [
            ("name", "SVG path length bridge name allocation failed"),
            (
                "callback",
                "SVG path length bridge callback allocation failed",
            ),
        ] {
            assert_eq!(
                require_bridge_handle::<()>(None, "SVG path length", kind),
                Err(expected.to_owned()),
            );
        }
    }
}
