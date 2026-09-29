use super::script::HtmlTryCatchScope;
use super::types::HtmlRuntimeError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/* WHY: Host scheduling on slower supported CPUs must not turn ordinary layout
 * work into a false execution timeout; the budget still bounds untrusted scripts. */
const EXECUTION_TIMEOUT: Duration = Duration::from_millis(250);
const HOST_IO_TIMEOUT: Duration = Duration::from_secs(2);

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
                EXECUTION_TIMEOUT,
                HOST_IO_TIMEOUT,
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
    execution_remaining: Duration,
    host_io_remaining: Duration,
    terminate_execution: impl Fn() + Send + 'static,
) {
    let mut execution_remaining = execution_remaining;
    let mut host_io_remaining = host_io_remaining;
    let mut observed_at = Instant::now();
    loop {
        if execution_remaining.is_zero() || host_io_remaining.is_zero() {
            timeout_marker.store(true, Ordering::SeqCst);
            terminate_execution();
            return;
        }
        if completion_received(&wait_for_completion, execution_remaining, host_io_remaining) {
            return;
        }
        observed_at = account_elapsed_budget(
            &host_io_active,
            &mut execution_remaining,
            &mut host_io_remaining,
            observed_at,
        );
    }
}

fn completion_received(
    wait_for_completion: &mpsc::Receiver<()>,
    execution_remaining: Duration,
    host_io_remaining: Duration,
) -> bool {
    wait_for_completion
        .recv_timeout(
            Duration::from_millis(1)
                .min(execution_remaining)
                .min(host_io_remaining),
        )
        .is_ok()
}

fn account_elapsed_budget(
    host_io_active: &AtomicBool,
    execution_remaining: &mut Duration,
    host_io_remaining: &mut Duration,
    observed_at: Instant,
) -> Instant {
    let now = Instant::now();
    let elapsed = now.duration_since(observed_at);
    if host_io_active.load(Ordering::SeqCst) {
        /* WHY: 同期DOM処理の遅さは通常のJS予算から分離するが、ページが
         * DOM callbackを反復して実行時間を無制限に延ばせないよう別枠を設ける。 */
        *host_io_remaining = host_io_remaining.saturating_sub(elapsed);
    } else {
        *execution_remaining = execution_remaining.saturating_sub(elapsed);
    }
    now
}

#[cfg(test)]
mod tests {
    use super::wait_for_timeout;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    #[test]
    fn host_io_pause_has_a_separate_finite_budget() {
        let active = Arc::new(AtomicBool::new(true));
        let timed_out = Arc::new(AtomicBool::new(false));
        let did_terminate = Arc::new(AtomicBool::new(false));
        let terminated = Arc::clone(&did_terminate);
        let (_completion, receiver) = mpsc::channel();

        wait_for_timeout(
            receiver,
            active,
            Arc::clone(&timed_out),
            Duration::from_secs(1),
            Duration::from_millis(20),
            move || terminated.store(true, Ordering::SeqCst),
        );

        assert!(timed_out.load(Ordering::SeqCst));
        assert!(did_terminate.load(Ordering::SeqCst));
    }
}
