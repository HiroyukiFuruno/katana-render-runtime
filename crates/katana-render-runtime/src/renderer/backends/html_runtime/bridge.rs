use super::dom_state::HtmlDomBridgeState;
use super::types::DomValue;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const MISSING_DOM_STATE_ERROR: &str = "HTML DOM state is unavailable";

struct HostOperationGuard {
    active: Arc<AtomicBool>,
    was_active: bool,
}

const NATIVE_DOM_ERROR_NAME: &str = "KrrNativeDomError";
const NATIVE_DOM_ERROR_PRIVATE_KEY: &str = "KatanA#NativeDomBridgeError";

impl HostOperationGuard {
    fn enter(active: Arc<AtomicBool>) -> Self {
        let was_active = active.swap(true, Ordering::SeqCst);
        Self { active, was_active }
    }
}

impl Drop for HostOperationGuard {
    fn drop(&mut self) {
        self.active.store(self.was_active, Ordering::SeqCst);
    }
}

pub(super) fn dom_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut return_value: v8::ReturnValue<v8::Value>,
) {
    /* WHY: 引数の文字列化はページのJavaScriptを呼べるため通常の実行予算で計測する。 */
    let operation = args.get(0).to_rust_string_lossy(scope);
    let arguments = (1..args.length())
        .map(|index| args.get(index).to_rust_string_lossy(scope))
        .collect::<Vec<_>>();
    /* WHY: 同期DOM処理だけを別枠で計測し、低速CPUで画像検証を誤検知せず、
     * ページの引数変換にはホスト処理用の実行猶予を与えない。 */
    let host_operation = scope
        .get_slot::<HtmlDomBridgeState>()
        .map(HtmlDomBridgeState::host_io_active)
        .map(HostOperationGuard::enter);
    let result = scope
        .get_slot::<HtmlDomBridgeState>()
        .ok_or(MISSING_DOM_STATE_ERROR.to_string())
        .and_then(|state| state.dispatch(&operation, &arguments));
    drop(host_operation);
    match result {
        Ok(value) => return_value.set(dom_value(scope, value)),
        Err(error) => set_bridge_error(scope, &mut return_value, error),
    }
}

fn set_bridge_error(
    scope: &mut v8::PinScope,
    return_value: &mut v8::ReturnValue<v8::Value>,
    error: String,
) {
    let Some(state) = scope.get_slot::<HtmlDomBridgeState>() else {
        return_value.set(v8::undefined(scope).into());
        return;
    };
    let _previous_error = state.error.replace(Some(error.clone()));
    let Some(message) = v8::String::new(scope, &error) else {
        return_value.set(v8::undefined(scope).into());
        return;
    };
    let exception = v8::Exception::error(scope, message);
    mark_native_dom_error(scope, exception, message);
    return_value.set(scope.throw_exception(exception));
}

fn native_dom_error_key<'scope>(
    scope: &mut v8::PinScope<'scope, '_>,
) -> Option<v8::Local<'scope, v8::Private>> {
    let name = v8::String::new(scope, NATIVE_DOM_ERROR_PRIVATE_KEY)?;
    Some(v8::Private::for_api(scope, Some(name)))
}

fn mark_native_dom_error<'scope>(
    scope: &mut v8::PinScope<'scope, '_>,
    exception: v8::Local<'scope, v8::Value>,
    message: v8::Local<'scope, v8::String>,
) {
    let (Some(property), Some(name), Some(object)) = (
        v8::String::new(scope, "name"),
        v8::String::new(scope, NATIVE_DOM_ERROR_NAME),
        exception.to_object(scope),
    ) else {
        return;
    };
    let _named = object.define_own_property(
        scope,
        property.into(),
        name.into(),
        v8::PropertyAttribute::NONE,
    );
    if let Some(key) = native_dom_error_key(scope) {
        /* WHY: 印の設定に失敗しても例外はthrowし、非公開の印を確認できない例外はJS例外に留める。 */
        let _marked = object.set_private(scope, key, message.into());
    }
}

pub(super) fn uncaught_dom_error(
    scope: &mut super::script::HtmlTryCatchScope<'_, '_, '_, '_>,
) -> Option<String> {
    let property = v8::String::new(scope, "name")?;
    let expected = v8::String::new(scope, NATIVE_DOM_ERROR_NAME)?;
    /* WHY: プリミティブ例外を変換して、元の例外を別のTypeErrorで上書きしない。 */
    let exception = v8::Local::<v8::Object>::try_from(scope.exception()?).ok()?;
    let name = exception.get(scope, property.into())?;
    if !name.strict_equals(expected.into()) {
        return None;
    }
    let key = native_dom_error_key(scope)?;
    /* WHY: 元の文言を例外に結び付け、後続エラーやJSのmessage変更から切り離す。 */
    let message = v8::Local::<v8::String>::try_from(exception.get_private(scope, key)?).ok()?;
    let message = message.to_rust_string_lossy(scope);
    scope
        .get_slot::<HtmlDomBridgeState>()
        .and_then(|state| state.error.borrow_mut().take())
        .map(|_| message)
}

fn dom_value<'scope>(
    scope: &mut v8::PinScope<'scope, '_>,
    value: DomValue,
) -> v8::Local<'scope, v8::Value> {
    match value {
        DomValue::Undefined => v8::undefined(scope).into(),
        DomValue::Null => v8::null(scope).into(),
        DomValue::String(value) => {
            let undefined = v8::undefined(scope).into();
            v8::String::new(scope, &value).map_or(undefined, |value| value.into())
        }
        DomValue::NodeId(value) => {
            let undefined = v8::undefined(scope).into();
            v8::String::new(scope, &value.to_string()).map_or(undefined, |value| value.into())
        }
        DomValue::NodeIds(values) => node_id_array(scope, &values).into(),
    }
}

fn node_id_array<'scope>(
    scope: &mut v8::PinScope<'scope, '_>,
    values: &[u64],
) -> v8::Local<'scope, v8::Array> {
    let length = i32::try_from(values.len()).unwrap_or(i32::MAX);
    let array = v8::Array::new(scope, length);
    for (index, value) in values.iter().enumerate() {
        let _assigned = v8::String::new(scope, &value.to_string())
            .map(|value| array.set_index(scope, index as u32, value.into()));
    }
    array
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
