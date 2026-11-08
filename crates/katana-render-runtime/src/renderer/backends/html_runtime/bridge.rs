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
    let exception = scope.exception()?.to_object(scope)?;
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
mod tests {
    use super::*;
    use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
    use crate::renderer::backends::html_document::HtmlDocument;
    use crate::renderer::backends::html_runtime::script::HtmlTryCatchScope;
    use crate::renderer::backends::html_runtime::script::check_bridge_error;
    use crate::renderer::backends::html_runtime::script::{evaluate_value, install_dom_bridge};
    use rstest::rstest;

    macro_rules! with_v8_context {
        ($isolate:ident, $scope:ident, [$($prepare:stmt;)*], [$($setup:expr;)*], $body:block $(,)?) => {{
            DiagramV8Runtime::ensure_initialized();
            $($prepare)*
            let mut $isolate = v8::Isolate::new(Default::default());
            $(let _ = $setup;)*
            v8::scope!(let handle_scope, &mut $isolate);
            let context = v8::Context::new(handle_scope, Default::default());
            let context_scope = &mut v8::ContextScope::new(handle_scope, context);
            v8::tc_scope!(let $scope, &mut **context_scope);
            $body
        }};
    }

    fn call_unsupported_dom_callback(
        scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    ) -> Result<(bool, Option<String>), String> {
        let callback = v8::Function::new(scope, dom_callback)
            .ok_or("callback allocation failed".to_string())?;
        let operation: v8::Local<v8::Value> = v8::String::new(scope, "unsupported")
            .ok_or("operation allocation failed".to_string())?
            .into();
        let receiver = v8::undefined(scope).into();
        let returned = callback.call(scope, receiver, &[operation]);
        let exception_name = scope.exception().and_then(|exception| {
            let property = v8::String::new(scope, "name")?;
            exception
                .to_object(scope)?
                .get(scope, property.into())
                .map(|name| name.to_rust_string_lossy(scope))
        });
        Ok((
            returned.is_some_and(|value| value.is_undefined()),
            exception_name,
        ))
    }

    #[test]
    fn missing_dom_state_error_preserves_contract_message() {
        assert_eq!(MISSING_DOM_STATE_ERROR, "HTML DOM state is unavailable");
    }

    fn assert_unsupported_operation_error(
        returned_undefined: bool,
        exception_name: Option<String>,
        stored_error: Option<String>,
        runtime_error: Result<(), crate::renderer::backends::html_runtime::types::HtmlRuntimeError>,
    ) {
        assert!(!returned_undefined);
        assert_eq!(exception_name.as_deref(), Some(NATIVE_DOM_ERROR_NAME));
        assert_eq!(
            stored_error.as_deref(),
            Some("unsupported DOM operation: unsupported")
        );
        assert!(matches!(
            runtime_error,
            Err(crate::renderer::backends::html_runtime::types::HtmlRuntimeError::DomBridge(
                message
            )) if message == "unsupported DOM operation: unsupported"
        ));
    }

    #[test]
    fn dom_callback_stores_unsupported_operation_error() -> Result<(), String> {
        let error = with_v8_context!(
            isolate,
            scope,
            [let state = HtmlDomBridgeState::new(HtmlDocument::parse("<button id=action>Run</button>"));],
            [isolate.set_slot(state);],
            {
                let (returned_undefined, exception_name) =
                    call_unsupported_dom_callback(scope)?;
                let error = scope
                    .get_slot::<HtmlDomBridgeState>()
                    .and_then(|state| state.error.borrow().clone());
                let runtime_error = check_bridge_error(scope);
                Ok::<_, String>((returned_undefined, exception_name, error, runtime_error))
            }
        );
        let (returned_undefined, exception_name, error, runtime_error) = error?;
        assert_unsupported_operation_error(
            returned_undefined,
            exception_name,
            error,
            runtime_error,
        );
        Ok(())
    }

    #[test]
    fn failed_native_attribute_mutation_does_not_create_observer_record() -> Result<(), String> {
        let outcome = with_v8_context!(
            isolate,
            scope,
            [let state = HtmlDomBridgeState::new(HtmlDocument::parse("<button id=action>Run</button>"));],
            [isolate.set_slot(state);],
            {
                install_dom_bridge(scope, "about:blank").map_err(|error| error.to_string())?;
                let result = evaluate_value(
                    scope,
                    "failed-native-attribute-mutation",
                    "(() => { const target = document.getElementById('action'); const observer = new MutationObserver(() => {}); observer.observe(target, { attributes: true }); const returned = target.setAttribute('', 'x'); return returned === undefined && observer.takeRecords().length === 0; })()",
                )
                .map_err(|error| error.to_string())?
                .is_true();
                Ok::<_, String>((result, check_bridge_error(scope)))
            }
        )?;
        assert!(outcome.0);
        assert!(matches!(
            outcome.1,
            Err(crate::renderer::backends::html_runtime::types::HtmlRuntimeError::DomBridge(
                message
            )) if message == "attribute name is empty"
        ));
        Ok(())
    }

    #[test]
    fn dom_callback_without_state_returns_undefined() -> Result<(), String> {
        with_v8_context!(isolate, scope, [], [], {
            let (returned_undefined, exception_name) = call_unsupported_dom_callback(scope)?;
            assert!(returned_undefined);
            assert_eq!(exception_name, None);
        });
        Ok(())
    }

    #[test]
    fn callback_operation_and_argument_coercion_stay_in_javascript_budget() -> Result<(), String> {
        for expression in [
            "__krr_dom({toString(){active = __krr_host_active(); return 'unsupported';}})",
            "__krr_dom('unsupported', {toString(){active = __krr_host_active(); return 'arg';}})",
        ] {
            assert_callback_coercion(expression, false)?;
        }
        Ok(())
    }

    #[rstest]
    #[case::native(
        "__krr_dom('unsupported');",
        true,
        "unsupported DOM operation: unsupported"
    )]
    #[case::javascript("throw new Error('ordinary');", false, "ordinary")]
    #[case::caught_native_then_javascript(
        "try { __krr_dom('unsupported'); } catch (error) { if (error.name !== 'KrrNativeDomError') throw error; } throw new Error('ordinary');",
        false,
        "ordinary"
    )]
    #[case::forged_name(
        "const error = new Error('ordinary'); error.name = 'KrrNativeDomError'; throw error;",
        false,
        "ordinary"
    )]
    #[case::caught_native_then_forged_name(
        "try { __krr_dom('unsupported'); } catch (native) { const error = new Error('ordinary'); error.name = 'KrrNativeDomError'; throw error; }",
        false,
        "ordinary"
    )]
    #[case::rethrow_native(
        "try { __krr_dom('unsupported'); } catch (native) { throw native; }",
        true,
        "unsupported DOM operation: unsupported"
    )]
    #[case::rethrow_first_after_second(
        "let first; try { __krr_dom('unsupported'); } catch (error) { first = error; } try { __krr_dom('other'); } catch (error) {} throw first;",
        true,
        "unsupported DOM operation: unsupported"
    )]
    #[case::rethrow_second(
        "try { __krr_dom('other'); } catch (error) {} try { __krr_dom('unsupported'); } catch (error) { throw error; }",
        true,
        "unsupported DOM operation: unsupported"
    )]
    #[case::two_native_then_forged_name(
        "try { __krr_dom('unsupported'); } catch (error) {} try { __krr_dom('other'); } catch (error) {} const error = new Error('ordinary'); error.name = 'KrrNativeDomError'; throw error;",
        false,
        "ordinary"
    )]
    #[case::native_message_changed(
        "try { __krr_dom('unsupported'); } catch (error) { error.message = 'ordinary'; if (error.message !== 'ordinary') throw new Error('message assignment failed'); throw error; }",
        true,
        "unsupported DOM operation: unsupported"
    )]
    #[case::native_name_changed(
        "try { __krr_dom('unsupported'); } catch (error) { error.name = 'OtherError'; error.message = 'ordinary'; if (error.name !== 'OtherError' || error.message !== 'ordinary') throw new Error('property assignment failed'); throw error; }",
        false,
        "OtherError"
    )]
    fn uncaught_bridge_and_javascript_errors_keep_distinct_types(
        #[case] source: &str,
        #[case] native: bool,
        #[case] expected_message: &str,
    ) -> Result<(), String> {
        assert_script_error_classification(source, native, expected_message)
    }

    fn assert_script_error_classification(
        source: &str,
        native: bool,
        expected_message: &str,
    ) -> Result<(), String> {
        with_v8_context!(
            isolate,
            scope,
            [],
            [isolate.set_slot(HtmlDomBridgeState::new(HtmlDocument::parse("<p>Test</p>")));],
            {
                install_dom_bridge(scope, "about:blank").map_err(|error| error.to_string())?;
                let result = evaluate_value(scope, "bridge-error-classification", source);
                assert_error_classification(result.as_ref().err(), native, expected_message);
                if native {
                    assert!(check_bridge_error(scope).is_ok());
                }
                Ok(())
            },
        )
    }

    fn assert_error_classification(
        error: Option<&super::super::types::HtmlRuntimeError>,
        native: bool,
        expected_message: &str,
    ) {
        if native {
            assert!(
                matches!(
                    error,
                    Some(super::super::types::HtmlRuntimeError::DomBridge(message))
                        if message == expected_message
                ),
                "actual error: {error:?}"
            );
        } else {
            assert!(
                matches!(
                    error,
                    Some(super::super::types::HtmlRuntimeError::JavaScriptException(message))
                        if message.contains(expected_message)
                ),
                "actual error: {error:?}"
            );
        }
    }

    #[test]
    fn callback_coercion_cannot_use_host_timeout_allowance() -> Result<(), String> {
        for expression in [
            "__krr_dom({toString(){const end = Date.now() + 750; while(Date.now() < end) {} return 'unsupported';}})",
            "__krr_dom('unsupported', {toString(){const end = Date.now() + 750; while(Date.now() < end) {} return 'arg';}})",
        ] {
            assert_callback_coercion(expression, true)?;
        }
        Ok(())
    }

    fn assert_callback_coercion(expression: &str, expect_timeout: bool) -> Result<(), String> {
        with_v8_context!(
            isolate,
            scope,
            [],
            [isolate.set_slot(HtmlDomBridgeState::new(HtmlDocument::parse("<p>Test</p>")));],
            {
                assert!(install_dom_bridge(scope, "about:blank").is_ok());
                install_host_activity_probe(scope)?;
                let source = format!(
                    "let active = true; let nativeError = false; try {{ {expression}; }} catch (error) {{ if (error.name !== '{NATIVE_DOM_ERROR_NAME}') throw error; nativeError = true; }} if (!nativeError) throw new Error('missing native error'); active"
                );
                let result = evaluate_value(scope, "bridge-coercion-budget", &source);
                let timed_out = matches!(
                    result,
                    Err(super::super::types::HtmlRuntimeError::ExecutionTimeout)
                );
                assert_eq!(timed_out, expect_timeout, "{result:?}");
                if !expect_timeout {
                    assert!(result.is_ok_and(|value| value.is_false()));
                }
                assert!(
                    scope
                        .get_slot::<HtmlDomBridgeState>()
                        .is_some_and(|state| !state.host_io_active().load(Ordering::SeqCst))
                );
                Ok(())
            },
        )
    }

    fn install_host_activity_probe(scope: &mut v8::PinScope) -> Result<(), String> {
        let name = v8::String::new(scope, "__krr_host_active").ok_or("name allocation failed")?;
        let callback =
            v8::Function::new(scope, host_active_for_test).ok_or("function allocation failed")?;
        let context = scope.get_current_context();
        let global = context.global(scope);
        assert_eq!(global.set(scope, name.into(), callback.into()), Some(true));
        Ok(())
    }

    fn host_active_for_test(
        scope: &mut v8::PinScope,
        _args: v8::FunctionCallbackArguments,
        mut return_value: v8::ReturnValue<v8::Value>,
    ) {
        let active = scope
            .get_slot::<HtmlDomBridgeState>()
            .is_some_and(|state| state.host_io_active().load(Ordering::SeqCst));
        return_value.set(v8::Boolean::new(scope, active).into());
    }
}
