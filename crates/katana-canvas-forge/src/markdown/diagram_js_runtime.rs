use std::borrow::Cow;
use std::sync::OnceLock;

static V8_INIT: OnceLock<Result<(), String>> = OnceLock::new();
const SOURCE_ALLOC_ERR: &str = "source allocation failed";
const FILENAME_ALLOC_ERR: &str = "filename allocation failed";

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
        Self::ensure_initialized()?;

        let mut isolate = v8::Isolate::new(Default::default());
        let handle_scope = &mut v8::HandleScope::new(&mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let scope = &mut v8::ContextScope::new(handle_scope, context);
        let scope = &mut v8::TryCatch::new(scope);

        let mut rendered = String::new();
        for script in scripts {
            rendered = evaluate(scope, script)?;
        }
        Ok(rendered)
    }

    fn ensure_initialized() -> Result<(), String> {
        V8_INIT
            .get_or_init(|| {
                /* WHY: mathjax_svg and rusty_v8 share the same global V8 state.
                Keep a single initialization owner to avoid panic during later math rendering. */
                shared_runtime_init_result(mathjax_svg::convert_to_svg_inline("x"))
            })
            .clone()
    }
}

fn evaluate(
    scope: &mut v8::TryCatch<v8::HandleScope>,
    script: &DiagramRuntimeScript<'_>,
) -> Result<String, String> {
    let script_code = script.code.as_ref();
    let source = required(v8::String::new(scope, script_code), SOURCE_ALLOC_ERR)?;
    let origin_name = required(v8::String::new(scope, script.name), FILENAME_ALLOC_ERR)?;
    let origin = script_origin(scope, origin_name);
    let script = v8::Script::compile(scope, source, Some(&origin))
        .ok_or_else(|| exception_message(scope))?;
    let value = script.run(scope).ok_or_else(|| exception_message(scope))?;
    resolve_value(scope, value)
}

fn resolve_value(
    scope: &mut v8::TryCatch<v8::HandleScope>,
    value: v8::Local<v8::Value>,
) -> Result<String, String> {
    let Ok(promise) = v8::Local::<v8::Promise>::try_from(value) else {
        return Ok(value.to_rust_string_lossy(scope));
    };
    scope.perform_microtask_checkpoint();
    match promise.state() {
        v8::PromiseState::Fulfilled => Ok(promise.result(scope).to_rust_string_lossy(scope)),
        v8::PromiseState::Rejected => Err(promise.result(scope).to_rust_string_lossy(scope)),
        v8::PromiseState::Pending => Err("Diagram render Promise did not settle".to_string()),
    }
}

fn script_origin<'a>(
    scope: &mut v8::TryCatch<v8::HandleScope<'a>>,
    origin_name: v8::Local<'a, v8::String>,
) -> v8::ScriptOrigin<'a> {
    v8::ScriptOrigin::new(
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
    )
}

fn exception_message(scope: &mut v8::TryCatch<v8::HandleScope>) -> String {
    let Some(exception) = scope.exception() else {
        return "unknown V8 exception".to_string();
    };
    exception.to_rust_string_lossy(scope)
}

fn required<T>(value: Option<T>, message: &'static str) -> Result<T, String> {
    value.ok_or_else(|| message.to_string())
}

fn shared_runtime_init_result(
    result: Result<String, impl std::fmt::Display>,
) -> Result<(), String> {
    result
        .map(|_| ())
        .map_err(|err| format!("Failed to initialize shared V8 runtime: {err}"))
}

#[cfg(test)]
#[path = "diagram_js_runtime_tests.rs"]
mod tests;
