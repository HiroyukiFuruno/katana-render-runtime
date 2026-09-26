use super::script::HtmlTryCatchScope;
use super::types::HtmlRuntimeError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/* WHY: Host scheduling on slower supported CPUs must not turn ordinary layout
 * work into a false execution timeout; the budget still bounds untrusted scripts. */
const EXECUTION_TIMEOUT: Duration = Duration::from_millis(250);

pub(super) struct ExecutionBudget {
    completion: mpsc::Sender<()>,
    timed_out: Arc<AtomicBool>,
    timer: JoinHandle<()>,
}

impl ExecutionBudget {
    pub(super) fn start(scope: &HtmlTryCatchScope<'_, '_, '_, '_>) -> Self {
        let isolate = scope.thread_safe_handle();
        let host_io_active = scope
            .get_slot::<super::dom_state::HtmlDomBridgeState>()
            .map(super::dom_state::HtmlDomBridgeState::host_io_active)
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
        let (completion, wait_for_completion) = mpsc::channel();
        let timed_out = Arc::new(AtomicBool::new(false));
        let timeout_marker = Arc::clone(&timed_out);
        let terminate_execution = move || {
            isolate.terminate_execution();
        };
        let timer = std::thread::spawn(move || {
            wait_for_timeout(
                wait_for_completion,
                host_io_active,
                timeout_marker,
                terminate_execution,
            )
        });
        Self {
            completion,
            timed_out,
            timer,
        }
    }

    pub(super) fn finish(self) -> Result<(), HtmlRuntimeError> {
        let _ = self.completion.send(());
        self.timer
            .join()
            .map_err(|_| HtmlRuntimeError::ExecutionTimeout)?;
        if self.timed_out.load(Ordering::SeqCst) {
            return Err(HtmlRuntimeError::ExecutionTimeout);
        }
        Ok(())
    }
}

fn wait_for_timeout(
    wait_for_completion: mpsc::Receiver<()>,
    host_io_active: Arc<AtomicBool>,
    timeout_marker: Arc<AtomicBool>,
    terminate_execution: impl Fn() + Send + 'static,
) {
    let mut remaining = EXECUTION_TIMEOUT;
    let mut observed_at = Instant::now();
    loop {
        if wait_for_completion
            .recv_timeout(Duration::from_millis(1).min(remaining))
            .is_ok()
        {
            return;
        }
        let now = Instant::now();
        if !host_io_active.load(Ordering::SeqCst) {
            remaining = remaining.saturating_sub(now.duration_since(observed_at));
        }
        observed_at = now;
        if remaining.is_zero() {
            timeout_marker.store(true, Ordering::SeqCst);
            terminate_execution();
            return;
        }
    }
}
