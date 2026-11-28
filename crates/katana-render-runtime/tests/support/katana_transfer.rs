use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

pub(super) type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

static ENV_LOCK: Mutex<()> = Mutex::new(());

pub(super) fn missing_runtime_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kdr-{name}-missing-{}.js", std::process::id()))
}

pub(super) fn env_guard() -> TestResult<MutexGuard<'static, ()>> {
    ENV_LOCK
        .lock()
        .map_err(|error| test_error(format!("environment lock is poisoned: {error}")))
}

pub(super) fn test_error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message.into()))
}
