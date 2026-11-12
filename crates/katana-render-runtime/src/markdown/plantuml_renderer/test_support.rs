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
    use crate::system::ProcessService;
    use std::panic::{catch_unwind, resume_unwind};

    const CHILD_CASE: &str = "KRR_PLANTUML_ENV_LOCK_TEST_CHILD";
    const CASE_NAME: &str = "markdown::plantuml_renderer::test_support::lock_tests::poisoned_environment_lock_is_reported_in_isolated_child";

    #[test]
    fn poisoned_environment_lock_is_reported_in_isolated_child()
    -> Result<(), Box<dyn std::error::Error>> {
        /* WHY: 共有ロックのpoisonをこの子プロセスだけに閉じ込める。 */
        if ProcessService::enter_isolated_test_child(CHILD_CASE, CASE_NAME)? {
            assert_poison_reported();
        } else {
            assert!(env_guard().is_ok());
        }
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
