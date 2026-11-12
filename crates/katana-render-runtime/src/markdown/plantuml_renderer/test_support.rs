use std::sync::MutexGuard;

#[path = "test_env_override.rs"]
mod test_env_override;
pub(super) use test_env_override::EnvOverride;

impl EnvOverride {
    pub(super) fn set_path(key: &'static str, value: &std::path::Path) -> Self {
        Self::set(key, value)
    }
}

pub(super) fn env_guard() -> Result<MutexGuard<'static, ()>, String> {
    super::asset::PLANTUML_ENV_LOCK
        .lock()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod lock_tests {
    use super::env_guard;
    use std::panic::{catch_unwind, resume_unwind};

    const CHILD_CASE: &str = "KRR_PLANTUML_ENV_LOCK_TEST_CHILD";
    const CASE_NAME: &str = "markdown::plantuml_renderer::test_support::lock_tests::poisoned_environment_lock_is_reported_in_isolated_child";
    const CHILD_ARGS: [&str; 3] = ["--exact", CASE_NAME, "--test-threads=1"];

    #[test]
    fn poisoned_environment_lock_is_reported_in_isolated_child()
    -> Result<(), Box<dyn std::error::Error>> {
        if let Some(child) = std::env::var_os(CHILD_CASE) {
            assert_eq!(child, CASE_NAME);
            assert!(std::env::args().skip(1).eq(CHILD_ARGS));
            assert_poison_reported();
            return Ok(());
        }

        /* WHY: 共有ロックのpoisonをこの子プロセスだけに閉じ込める。 */
        let output = crate::system::ProcessService::create_command(std::env::current_exe()?)
            .args(CHILD_ARGS)
            .env(CHILD_CASE, CASE_NAME)
            .output()?;
        let stdout = std::str::from_utf8(&output.stdout)?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("test result: ok. 1 passed;"), "{stdout}");
        assert!(env_guard().is_ok());
        Ok(())
    }

    fn assert_poison_reported() {
        let interrupted = catch_unwind(|| -> Result<(), String> {
            let _guard = env_guard()?;
            resume_unwind(Box::new(
                "interrupt isolated PlantUML environment operation",
            ))
        });
        assert!(interrupted.is_err());
        assert!(env_guard().is_err_and(|error| error.contains("poisoned lock")));
    }
}
