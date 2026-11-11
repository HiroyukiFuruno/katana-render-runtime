pub(super) fn register_callback<'scope>(
    scope: &mut v8::PinScope<'scope, '_>,
    global: v8::Local<v8::Object>,
    name: &str,
    callback: impl v8::MapFnTo<v8::FunctionCallback>,
    label: &str,
) -> Result<
    (
        v8::Local<'scope, v8::String>,
        v8::Local<'scope, v8::Function>,
    ),
    String,
> {
    let name = v8::String::new(scope, name)
        .ok_or_else(|| format!("{label} bridge name allocation failed"))?;
    let callback = v8::Function::new(scope, callback)
        .ok_or_else(|| format!("{label} bridge callback allocation failed"))?;
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
