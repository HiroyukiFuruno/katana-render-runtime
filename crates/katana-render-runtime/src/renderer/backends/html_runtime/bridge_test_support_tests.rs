use std::sync::atomic::Ordering;

use super::super::{NATIVE_DOM_ERROR_NAME, dom_callback};
use super::{
    DiagramV8Runtime, HtmlDocument, HtmlDomBridgeState, HtmlTryCatchScope, check_bridge_error,
    evaluate_value, install_dom_bridge,
};

#[test]
fn marking_a_primitive_as_a_native_error_does_not_throw() -> Result<(), String> {
    with_v8_context!(isolate, scope, [], [], {
        let message = v8::String::new(scope, "native error").ok_or("message unavailable")?;
        let primitive = v8::null(scope).into();
        super::super::mark_native_dom_error(scope, primitive, message);
        assert!(!scope.has_caught());
    });
    Ok(())
}

pub(super) fn call_unsupported_dom_callback(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
) -> Result<(bool, Option<String>), String> {
    let callback =
        v8::Function::new(scope, dom_callback).ok_or("callback allocation failed".to_string())?;
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

pub(super) fn assert_unsupported_operation_error(
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

pub(super) fn assert_script_error_classification(
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
    error: Option<&super::super::super::types::HtmlRuntimeError>,
    native: bool,
    expected_message: &str,
) {
    if native {
        assert!(
            matches!(
                error,
                Some(super::super::super::types::HtmlRuntimeError::DomBridge(message))
                    if message == expected_message
            ),
            "actual error: {error:?}"
        );
    } else {
        assert!(
            matches!(
                error,
                Some(super::super::super::types::HtmlRuntimeError::JavaScriptException(message))
                    if message.contains(expected_message)
            ),
            "actual error: {error:?}"
        );
    }
}

pub(super) fn assert_callback_coercion(
    expression: &str,
    expect_timeout: bool,
) -> Result<(), String> {
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
                Err(super::super::super::types::HtmlRuntimeError::ExecutionTimeout)
            );
            assert_eq!(timed_out, expect_timeout, "{result:?}");
            if !expect_timeout {
                assert!(result.is_ok_and(|value| value.is_false()));
            }
            assert_host_operation_inactive(scope);
            Ok(())
        },
    )
}

fn assert_host_operation_inactive(scope: &HtmlTryCatchScope<'_, '_, '_, '_>) {
    assert!(
        scope
            .get_slot::<HtmlDomBridgeState>()
            .is_some_and(|state| !state.host_io_active().load(Ordering::SeqCst))
    );
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

#[test]
fn successful_dom_callback_mutates_the_element_and_returns_undefined() -> Result<(), String> {
    with_v8_context!(
        isolate,
        scope,
        [],
        [isolate.set_slot(HtmlDomBridgeState::new(HtmlDocument::parse("<button id=action>Run</button>")));],
        {
            install_dom_bridge(scope, "about:blank").map_err(|error| error.to_string())?;
            let result = evaluate_value(
                scope,
                "successful-native-attribute-mutation",
                "(() => { const id = __krr_dom('getElementById', 'action'); const result = __krr_dom('setAttribute', id, 'title', 'Updated'); return result === undefined && document.getElementById('action').getAttribute('title') === 'Updated'; })()",
            )
            .map_err(|error| error.to_string())?;
            assert!(result.is_true());
            check_bridge_error(scope).map_err(|error| error.to_string())
        }
    )
}
