use std::path::Path;

pub(super) struct EnvOverride {
    key: &'static str,
}

impl EnvOverride {
    pub(super) fn set(key: &'static str, value: &Path) -> Self {
        unsafe { std::env::set_var(key, value) };
        Self { key }
    }
}

impl Drop for EnvOverride {
    fn drop(&mut self) {
        unsafe { std::env::remove_var(self.key) };
    }
}
