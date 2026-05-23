use super::super::asset::PlantUmlJarAssetOps;
use super::PlantUmlRuntimePathOps;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn java_home_candidates_include_server_libjvm() {
    let candidates = PlantUmlRuntimePathOps::java_home_jvm_candidates("jdk".as_ref());

    assert!(
        candidates
            .iter()
            .any(|it| it.ends_with("lib/server/libjvm.dylib")
                || it.ends_with("lib/server/libjvm.so")
                || it.ends_with("bin/server/jvm.dll"))
    );
}

#[test]
fn missing_paths_create_actionable_warning() {
    let result = PlantUmlRuntimePathOps::resolve_existing_jar("missing.jar".as_ref(), None);

    assert!(matches!(
        result,
        Err(warning) if warning.message().contains("plantuml-runtime-unavailable")
            && warning.message().contains("KRR_PLANTUML_CACHE_DIR")
            && warning.message().contains("network access")
    ));
}

#[test]
fn api_cache_dir_overrides_default_cache_path() {
    if std::env::var_os("KRR_PLANTUML_JAR").is_some()
        || std::env::var_os("KDR_PLANTUML_JAR").is_some()
        || std::env::var_os("PLANTUML_JAR").is_some()
    {
        return;
    }
    let default_path = PlantUmlJarAssetOps::cache_path(None);
    let cache_dir = PathBuf::from("/tmp/krr-api-cache");
    let effective =
        PlantUmlRuntimePathOps::effective_jar_path(&default_path, Some(cache_dir.as_path()));

    assert_eq!(
        effective,
        PlantUmlJarAssetOps::cache_path(Some(cache_dir.as_path()))
    );
}

#[test]
fn jar_env_prefers_krr_over_kdr() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr = EnvOverride::set("KRR_PLANTUML_JAR", "/tmp/krr.jar");
    let _kdr = EnvOverride::set("KDR_PLANTUML_JAR", "/tmp/kdr.jar");
    let _plantuml = EnvOverride::set("PLANTUML_JAR", "/tmp/plantuml.jar");

    assert_eq!(
        PlantUmlRuntimePathOps::surface_jar_path(),
        PathBuf::from("/tmp/krr.jar")
    );
    Ok(())
}

#[test]
fn jar_env_uses_kdr_when_krr_is_missing() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr = EnvOverride::unset("KRR_PLANTUML_JAR");
    let _kdr = EnvOverride::set("KDR_PLANTUML_JAR", "/tmp/kdr.jar");
    let _plantuml = EnvOverride::set("PLANTUML_JAR", "/tmp/plantuml.jar");

    assert_eq!(
        PlantUmlRuntimePathOps::surface_jar_path(),
        PathBuf::from("/tmp/kdr.jar")
    );
    Ok(())
}

#[test]
fn jvm_env_prefers_krr_over_kdr() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr = EnvOverride::set("KRR_PLANTUML_JVM", "/tmp/krr-jvm");
    let _kdr = EnvOverride::set("KDR_PLANTUML_JVM", "/tmp/kdr-jvm");

    let candidates = PlantUmlRuntimePathOps::jvm_candidates();

    assert_eq!(candidates.first(), Some(&PathBuf::from("/tmp/krr-jvm")));
    assert_eq!(candidates.get(1), Some(&PathBuf::from("/tmp/kdr-jvm")));
    Ok(())
}

#[test]
fn missing_libjvm_create_actionable_warning() {
    let result = PlantUmlRuntimePathOps::resolve_jvm_from_candidates(vec![PathBuf::from(
        "target/krr tests/missing libjvm.dylib",
    )]);

    assert!(matches!(
        result,
        Err(warning) if warning.message().contains("libjvm was not found")
            && warning.message().contains("KRR_PLANTUML_JVM")
            && warning.message().contains("JAVA_HOME")
            && warning.message().contains("target/krr tests/missing libjvm.dylib")
            && warning.message().contains("install a JDK")
    ));
}

#[test]
fn jar_path_with_spaces_is_reported_without_shell_splitting() {
    let result = PlantUmlRuntimePathOps::resolve_existing_jar(
        "target/krr tests/missing jar.jar".as_ref(),
        None,
    );

    assert!(matches!(
        result,
        Err(warning) if warning.message().contains("target/krr tests/missing jar.jar")
    ));
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
