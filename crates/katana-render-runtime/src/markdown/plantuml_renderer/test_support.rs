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
