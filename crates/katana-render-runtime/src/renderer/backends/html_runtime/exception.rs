use super::script::HtmlTryCatchScope;

pub(super) fn exception_message(scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>) -> String {
    let Some(exception) = scope.exception() else {
        return unknown_v8_exception_message();
    };
    let summary = exception.to_rust_string_lossy(scope);
    let stack = scope
        .stack_trace()
        .map(|stack| stack.to_rust_string_lossy(scope))
        .filter(|stack| !stack.trim().is_empty() && stack != &summary);
    stack.unwrap_or_else(|| exception_location(scope, summary))
}

fn exception_location(scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>, summary: String) -> String {
    let context = scope.message().and_then(|message| {
        let line = message.get_line_number(scope)?;
        let column = message.get_start_column() + 1;
        let resource = message
            .get_script_resource_name(scope)
            .map(|value| value.to_rust_string_lossy(scope))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "inline-script".to_string());
        let source = message
            .get_source_line(scope)
            .map(|value| value.to_rust_string_lossy(scope))
            .filter(|value| !value.trim().is_empty())
            .map(|source| format!("\n  {source}"))
            .unwrap_or_else(String::new);
        Some(format!("  at {resource}:{line}:{column}{source}"))
    });
    context
        .map(|context| format!("{summary}\n{context}"))
        .unwrap_or(summary)
}

fn unknown_v8_exception_message() -> String {
    "unknown V8 exception".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exception_message_reports_empty_try_catch() {
        crate::markdown::diagram_js_runtime::DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);

        assert_eq!(exception_message(scope), "unknown V8 exception");
    }
}
