use super::{DiagramRuntimeScript, DiagramV8Runtime, exception_message, required};

#[test]
fn renders_plain_script_value() {
    let output = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed("plain.js", "'ok'")]);
    assert!(output.as_ref().is_ok_and(|it| it == "ok"));
}

#[test]
fn renders_fulfilled_and_pending_promise_values() {
    let fulfilled = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "promise.js",
        "Promise.resolve('done')",
    )]);
    let pending = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "pending.js",
        "new Promise(() => {})",
    )]);

    assert!(fulfilled.as_ref().is_ok_and(|it| it == "done"));
    assert!(matches!(pending, Err(message) if message.contains("did not settle")));
}

#[test]
fn returns_compile_and_rejected_promise_errors_with_messages() {
    assert_runtime_error("bad.js", "}", |it| !it.is_empty());
    assert_runtime_error("reject.js", "Promise.reject(new Error('rejected'))", |it| {
        it.contains("rejected")
    });
}

#[test]
fn returns_thrown_primitive_errors_with_messages() {
    assert_runtime_error("throw.js", "throw 'plain failure'", |it| {
        it.contains("plain failure")
    });
    assert_runtime_error("throw-null.js", "throw null", |it| it == "null");
}

#[test]
fn returns_thrown_object_errors_with_messages() {
    assert_runtime_error(
        "throw-same-stack.js",
        "throw { toString() { return 'same'; }, stack: 'same' }",
        |it| it == "same",
    );
    assert_runtime_error(
        "throw-without-stack.js",
        "throw { toString() { return 'without stack'; } }",
        |it| it == "without stack",
    );
}

fn assert_runtime_error(
    script_name: &'static str,
    script: &'static str,
    matches_error: impl FnOnce(&str) -> bool,
) {
    let output = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(script_name, script)]);

    assert!(matches!(output, Err(message) if matches_error(&message)));
}

#[test]
fn exception_message_reports_empty_try_catch() {
    let _ = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed("init.js", "'ok'")]);
    let mut isolate = v8::Isolate::new(Default::default());
    v8::scope!(let handle_scope, &mut isolate);
    let context = v8::Context::new(handle_scope, Default::default());
    let context_scope = &mut v8::ContextScope::new(handle_scope, context);
    v8::tc_scope!(let scope, &mut **context_scope);

    assert_eq!(exception_message(scope), "unknown V8 exception");
}

#[test]
fn allocation_helper_is_error_first() {
    let missing = required::<()>(None, "missing value");

    assert!(matches!(missing, Err(message) if message == "missing value"));
    assert!(required(Some("ok"), "missing").is_ok());
}
