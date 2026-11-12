use super::{HtmlDomBridgeState, MISSING_DOM_STATE_ERROR};
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

#[path = "bridge_test_support_tests.rs"]
mod support;

use support::{
    assert_callback_coercion, assert_script_error_classification,
    assert_unsupported_operation_error, call_unsupported_dom_callback,
};

#[test]
fn missing_dom_state_error_preserves_contract_message() {
    assert_eq!(MISSING_DOM_STATE_ERROR, "HTML DOM state is unavailable");
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
    assert_unsupported_operation_error(returned_undefined, exception_name, error, runtime_error);
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
#[case::object_inheriting_native_error(
    "try { __krr_dom('unsupported'); } catch (native) { throw Object.create(native); }",
    false,
    "undefined"
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
#[case::native_name_getter_throws(
    "try { __krr_dom('unsupported'); } catch (error) { error.toString = () => 'name getter failure'; Object.defineProperty(error, 'name', { get() { throw new Error('name getter failure'); } }); throw error; }",
    false,
    "name getter failure"
)]
fn uncaught_bridge_and_javascript_errors_keep_distinct_types(
    #[case] source: &str,
    #[case] native: bool,
    #[case] expected_message: &str,
) -> Result<(), String> {
    assert_script_error_classification(source, native, expected_message)
}

#[rstest]
#[case::null("null")]
#[case::undefined("undefined")]
#[case::number("42")]
fn thrown_primitive_keeps_the_original_javascript_message(
    #[case] literal: &str,
) -> Result<(), String> {
    assert_ordinary_exception_summary(&format!("throw {literal};"), literal)
}

#[test]
fn ordinary_thrown_object_does_not_run_name_getter_during_classification() -> Result<(), String> {
    let source = "let getterUsed = false; throw { toString() { return getterUsed ? 'ordinary after getter' : 'ordinary original'; }, get name() { getterUsed = true; throw new Error('name getter was run'); } };";
    assert_ordinary_exception_summary(source, "ordinary original")
}

#[test]
fn ordinary_proxy_does_not_run_name_trap_during_classification() -> Result<(), String> {
    let source = "let trapUsed = false; throw new Proxy({ toString() { return trapUsed ? 'proxy after trap' : 'ordinary proxy'; } }, { get(target, property, receiver) { if (property === 'name') { trapUsed = true; throw new Error('name trap was run'); } return Reflect.get(target, property, receiver); } });";
    assert_ordinary_exception_summary(source, "ordinary proxy")
}

fn assert_ordinary_exception_summary(source: &str, expected: &str) -> Result<(), String> {
    use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;
    let error = with_v8_context!(isolate, scope, [], [], {
        evaluate_value(scope, "bridge-ordinary-exception", source).err()
    });
    let Some(HtmlRuntimeError::JavaScriptException(message)) = error else {
        return Err(format!("expected a JavaScript exception, got {error:?}"));
    };
    assert_eq!(message.lines().next(), Some(expected), "{message}");
    Ok(())
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
