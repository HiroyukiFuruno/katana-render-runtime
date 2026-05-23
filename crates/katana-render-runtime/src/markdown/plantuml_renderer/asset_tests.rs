use super::{
    PLANTUML_DOWNLOAD_URL, PLANTUML_JAR_CHECKSUM, PLANTUML_JAR_VERSION, PlantUmlJarAssetOps,
};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn plantuml_asset_metadata_is_pinned() {
    assert_eq!(PLANTUML_JAR_VERSION, "1.2026.4");
    assert_eq!(PLANTUML_JAR_CHECKSUM.len(), 64);
    assert!(PLANTUML_DOWNLOAD_URL.contains("plantuml-lgpl"));
}

#[test]
fn cache_path_uses_explicit_root_and_pinned_version() {
    assert_eq!(
        PlantUmlJarAssetOps::cache_path(Some(Path::new("/tmp/krr-cache"))),
        Path::new("/tmp/krr-cache")
            .join(PLANTUML_JAR_VERSION)
            .join("plantuml.jar")
    );
    assert_eq!(
        PlantUmlJarAssetOps::digest_bytes(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn default_cache_path_uses_krr_namespace() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr = EnvOverride::unset("KRR_PLANTUML_CACHE_DIR");
    let _kdr = EnvOverride::unset("KDR_PLANTUML_CACHE_DIR");
    let expected_suffix = PathBuf::from("krr")
        .join("plantuml")
        .join(PLANTUML_JAR_VERSION)
        .join("plantuml.jar");

    assert!(PlantUmlJarAssetOps::cache_path(None).ends_with(expected_suffix));
    Ok(())
}

#[test]
fn cache_env_prefers_krr_over_kdr() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr = EnvOverride::set("KRR_PLANTUML_CACHE_DIR", "/tmp/krr-cache");
    let _kdr = EnvOverride::set("KDR_PLANTUML_CACHE_DIR", "/tmp/kdr-cache");

    assert_eq!(
        PlantUmlJarAssetOps::cache_path(None),
        Path::new("/tmp/krr-cache")
            .join(PLANTUML_JAR_VERSION)
            .join("plantuml.jar")
    );
    Ok(())
}

struct EnvOverride {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvOverride {
    fn set(key: &'static str, value: &'static str) -> Self {
        let original = std::env::var_os(key);
        unsafe { std::env::set_var(key, value) };
        Self { key, original }
    }

    fn unset(key: &'static str) -> Self {
        let original = std::env::var_os(key);
        unsafe { std::env::remove_var(key) };
        Self { key, original }
    }
}

impl Drop for EnvOverride {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

fn env_guard() -> Result<MutexGuard<'static, ()>, String> {
    ENV_LOCK.lock().map_err(|error| error.to_string())
}
