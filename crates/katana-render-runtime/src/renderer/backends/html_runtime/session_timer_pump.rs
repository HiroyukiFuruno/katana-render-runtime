use crate::renderer::backends::html_runtime::evaluation::{
    compile_unbounded, evaluate_value_unbounded, run_unbounded,
};
use crate::renderer::backends::html_runtime::execution::ExecutionBudget;
use crate::renderer::backends::html_runtime::script::{HtmlTryCatchScope, check_bridge_error};
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;

impl super::StaticHtmlRuntimeSession {
    pub(crate) fn advance_timers(&mut self, elapsed_ms: f64) -> Result<bool, HtmlRuntimeError> {
        let isolate = self
            .isolate
            .as_mut()
            .ok_or_else(super::session_interaction::discarded_runtime_error)?;
        let context = self
            .context
            .as_ref()
            .ok_or_else(super::session_interaction::discarded_runtime_error)?;
        v8::scope!(let handle_scope, isolate);
        let context = v8::Local::new(handle_scope, context);
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        let elapsed = if elapsed_ms.is_finite() {
            elapsed_ms.max(0.0)
        } else {
            0.0
        };
        pump_timers(scope, elapsed)
    }
}

pub(super) fn pump_timers(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    elapsed: f64,
) -> Result<bool, HtmlRuntimeError> {
    let budget = ExecutionBudget::start(scope);
    let result = run_due_timers(scope, elapsed);
    budget.finish()?;
    result
}

fn run_due_timers(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    elapsed: f64,
) -> Result<bool, HtmlRuntimeError> {
    const SCRIPT_NAME: &str = "krr-html-timer-pump";
    let code = format!("globalThis.__krrRunTimers?.({elapsed});");
    let ran = evaluate_value_unbounded(scope, SCRIPT_NAME, &code)?.is_true();
    finish_timer_task(scope)?;
    if !ran {
        return Ok(false);
    }

    /* WHY: 時刻を進めるのは初回だけ。後続のタイマーは同じスクリプトを再利用する。 */
    let script = compile_unbounded(scope, SCRIPT_NAME, "globalThis.__krrRunTimers?.(0);")?;
    loop {
        let ran = run_unbounded(scope, script)?.is_true();
        finish_timer_task(scope)?;
        if !ran {
            return Ok(true);
        }
    }
}

fn finish_timer_task(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
) -> Result<(), HtmlRuntimeError> {
    scope.as_mut().perform_microtask_checkpoint();
    if scope.has_terminated() {
        return Err(HtmlRuntimeError::ExecutionTimeout);
    }
    check_bridge_error(scope)
}
