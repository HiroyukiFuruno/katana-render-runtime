use super::{HtmlBrowserError, HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static COLD_OPEN_COMPLETED: AtomicBool = AtomicBool::new(false);
static COLD_OPEN_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Public HTML runtime entry point for browser-equivalent interactive surfaces.
#[derive(Debug, Clone, Copy)]
pub struct HtmlRuntime;

pub type HtmlRuntimeSession = HtmlBrowserSession;

impl HtmlRuntime {
    pub fn open(
        &self,
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<HtmlRuntimeSession, HtmlBrowserError> {
        if COLD_OPEN_COMPLETED.load(Ordering::Acquire) {
            return HtmlBrowserSession::start_in_process(source, viewport);
        }

        let cold_open_lock = COLD_OPEN_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .map_err(|_| HtmlBrowserError::RuntimeFailure {
                error: "HTML runtime cold-start lock is unavailable".to_string(),
            })?;
        if COLD_OPEN_COMPLETED.load(Ordering::Acquire) {
            drop(cold_open_lock);
            return HtmlBrowserSession::start_in_process(source, viewport);
        }

        let session = HtmlBrowserSession::start_in_process(source, viewport);
        if session.is_ok() {
            COLD_OPEN_COMPLETED.store(true, Ordering::Release);
        }
        drop(cold_open_lock);
        session
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
