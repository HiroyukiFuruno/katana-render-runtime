use std::borrow::Cow;
use std::sync::OnceLock;

static V8_INIT: OnceLock<()> = OnceLock::new();
const SOURCE_ALLOC_ERR: &str = "source allocation failed";
const FILENAME_ALLOC_ERR: &str = "filename allocation failed";
type DiagramTryCatchScope<'pin, 'scope, 'object, 'isolate> =
    v8::PinnedRef<'pin, v8::TryCatch<'scope, 'object, v8::HandleScope<'isolate>>>;

pub(crate) struct DiagramRuntimeScript<'a> {
    pub(crate) name: &'static str,
    pub(crate) code: Cow<'a, str>,
}

impl<'a> DiagramRuntimeScript<'a> {
    pub(crate) fn borrowed(name: &'static str, code: &'a str) -> Self {
        Self {
            name,
            code: Cow::Borrowed(code),
        }
    }

    pub(crate) fn owned(name: &'static str, code: String) -> Self {
        Self {
            name,
            code: Cow::Owned(code),
        }
    }
}

pub(crate) struct DiagramV8Runtime;

impl DiagramV8Runtime {
    pub(crate) fn render(scripts: &[DiagramRuntimeScript<'_>]) -> Result<String, String> {
        Self::ensure_initialized();

        let mut isolate = v8::Isolate::new(Default::default());
        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);

        let mut rendered = String::new();
        for script in scripts {
            rendered = evaluate(scope, script)?;
        }
        Ok(rendered)
    }

    fn ensure_initialized() {
        V8_INIT.get_or_init(|| {
            let platform = v8::new_default_platform(0, false).make_shared();
            v8::V8::initialize_platform(platform);
            v8::V8::initialize();
        });
    }
}

fn evaluate(
    scope: &mut DiagramTryCatchScope<'_, '_, '_, '_>,
    script: &DiagramRuntimeScript<'_>,
) -> Result<String, String> {
    let script_code = script.code.as_ref();
    let source = required(v8::String::new(scope, script_code), SOURCE_ALLOC_ERR)?;
    let origin_name = required(v8::String::new(scope, script.name), FILENAME_ALLOC_ERR)?;
    let origin = v8::ScriptOrigin::new(
        scope,
        origin_name.into(),
        0,
        0,
        false,
        0,
        Some(origin_name.into()),
        false,
        false,
        false,
        None,
    );
    let script = v8::Script::compile(scope, source, Some(&origin))
        .ok_or_else(|| exception_message(scope))?;
    let value = script.run(scope).ok_or_else(|| exception_message(scope))?;
    resolve_value(scope, value)
}

fn resolve_value(
    scope: &mut DiagramTryCatchScope<'_, '_, '_, '_>,
    value: v8::Local<v8::Value>,
) -> Result<String, String> {
    let Ok(promise) = v8::Local::<v8::Promise>::try_from(value) else {
        return Ok(value.to_rust_string_lossy(scope));
    };
    drain_microtasks(scope, promise);
    match promise.state() {
        v8::PromiseState::Fulfilled => Ok(promise.result(scope).to_rust_string_lossy(scope)),
        v8::PromiseState::Rejected => Err(promise.result(scope).to_rust_string_lossy(scope)),
        v8::PromiseState::Pending => Err("Diagram render Promise did not settle".to_string()),
    }
}

fn drain_microtasks(
    scope: &mut DiagramTryCatchScope<'_, '_, '_, '_>,
    promise: v8::Local<'_, v8::Promise>,
) {
    for _ in 0..64 {
        scope.as_mut().perform_microtask_checkpoint();
        if promise.state() != v8::PromiseState::Pending {
            return;
        }
    }
}

fn exception_message(scope: &mut DiagramTryCatchScope<'_, '_, '_, '_>) -> String {
    let Some(exception) = scope.exception() else {
        return "unknown V8 exception".to_string();
    };
    exception.to_rust_string_lossy(scope)
}

fn required<T>(value: Option<T>, message: &'static str) -> Result<T, String> {
    value.ok_or_else(|| message.to_string())
}

#[cfg(test)]
#[path = "diagram_js_runtime_tests.rs"]
mod tests;
