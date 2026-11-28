use super::super::asset::PlantUmlJarAssetOps;
use super::super::test_support::{EnvOverride, env_guard};
use super::PlantUmlRuntimePathOps;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "resolve_runtime_paths_tests.rs"]
mod runtime_paths_tests;

static TEMP_ID: AtomicUsize = AtomicUsize::new(0);

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
fn api_cache_dir_overrides_default_cache_path() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr_jar = EnvOverride::unset("KRR_PLANTUML_JAR");
    let _kdr_jar = EnvOverride::unset("KDR_PLANTUML_JAR");
    let _plantuml_jar = EnvOverride::unset("PLANTUML_JAR");
    let _krr_cache = EnvOverride::unset("KRR_PLANTUML_CACHE_DIR");
    let _kdr_cache = EnvOverride::unset("KDR_PLANTUML_CACHE_DIR");
    let default_path = PlantUmlJarAssetOps::cache_path(None);
    let cache_dir = PathBuf::from("/tmp/krr-api-cache");
    let effective =
        PlantUmlRuntimePathOps::effective_jar_path(&default_path, Some(cache_dir.as_path()));

    assert_eq!(
        effective,
        PlantUmlJarAssetOps::cache_path(Some(cache_dir.as_path()))
    );
    Ok(())
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

#[test]
fn effective_jar_path_keeps_explicit_non_cache_path() {
    let explicit = PathBuf::from("/tmp/explicit-plantuml.jar");

    assert_eq!(
        PlantUmlRuntimePathOps::effective_jar_path(&explicit, None),
        explicit
    );
}

#[test]
fn existing_invalid_jar_reports_checksum_warning() -> Result<(), String> {
    let jar = temp_path("invalid.jar");
    std::fs::write(&jar, b"invalid").map_err(|error| error.to_string())?;

    let result = PlantUmlRuntimePathOps::resolve_existing_jar(&jar, None);

    assert!(matches!(result, Err(warning) if warning.message().contains("checksum mismatch")));
    Ok(())
}

#[test]
fn first_existing_candidate_and_jvm_resolution_use_existing_path() -> Result<(), String> {
    let existing = temp_path("libjvm.dylib");
    std::fs::write(&existing, b"placeholder").map_err(|error| error.to_string())?;
    let candidates = vec![PathBuf::from("missing-libjvm"), existing.clone()];

    assert_eq!(
        PlantUmlRuntimePathOps::first_existing(candidates.clone()),
        Some(existing.clone())
    );
    assert!(matches!(
        PlantUmlRuntimePathOps::resolve_jvm_from_candidates(candidates),
        Ok(path) if path == existing
    ));
    Ok(())
}

#[test]
fn environment_fallbacks_include_plantuml_and_java_home() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr_jar = EnvOverride::unset("KRR_PLANTUML_JAR");
    let _kdr_jar = EnvOverride::unset("KDR_PLANTUML_JAR");
    let _plantuml = EnvOverride::set("PLANTUML_JAR", "/tmp/plantuml-fallback.jar");
    let _krr_jvm = EnvOverride::unset("KRR_PLANTUML_JVM");
    let _kdr_jvm = EnvOverride::unset("KDR_PLANTUML_JVM");
    let _java_home = EnvOverride::set("JAVA_HOME", "/tmp/krr-test-jdk");

    let candidates = PlantUmlRuntimePathOps::jvm_candidates();

    assert_eq!(
        PlantUmlRuntimePathOps::surface_jar_path(),
        PathBuf::from("/tmp/plantuml-fallback.jar")
    );
    assert!(
        candidates
            .iter()
            .any(|path| path.starts_with("/tmp/krr-test-jdk"))
    );
    Ok(())
}

#[test]
fn jvm_candidates_allow_missing_java_home() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr_jvm = EnvOverride::unset("KRR_PLANTUML_JVM");
    let _kdr_jvm = EnvOverride::unset("KDR_PLANTUML_JVM");
    let _java_home = EnvOverride::unset("JAVA_HOME");

    let candidates = PlantUmlRuntimePathOps::jvm_candidates();

    assert!(
        candidates
            .iter()
            .all(|path| !path.starts_with("/tmp/krr-test-jdk"))
    );
    Ok(())
}

#[test]
fn verified_local_jar_is_returned_for_an_explicit_runtime_path() -> Result<(), String> {
    let jar_path = PlantUmlJarAssetOps::cache_path(None);
    if !jar_path.exists() {
        return Ok(());
    }

    let resolved = PlantUmlRuntimePathOps::resolve_existing_jar(
        &jar_path,
        Some(Path::new("/tmp/krr-unrelated-plantuml-cache")),
    )
    .map_err(|warning| warning.message())?;

    assert_eq!(resolved, jar_path);
    Ok(())
}

#[test]
fn resolve_paths_accepts_verified_jar_and_env_jvm() -> Result<(), String> {
    let jar_path = PlantUmlJarAssetOps::cache_path(None);
    if !jar_path.exists() {
        return Ok(());
    }
    let _guard = env_guard()?;
    let jvm_path = temp_path("libjvm.dylib");
    std::fs::write(&jvm_path, b"jvm").map_err(|error| error.to_string())?;
    let _krr_jvm = EnvOverride::set_path("KRR_PLANTUML_JVM", &jvm_path);
    let _kdr_jvm = EnvOverride::unset("KDR_PLANTUML_JVM");
    let _java_home = EnvOverride::unset("JAVA_HOME");

    let paths = PlantUmlRuntimePathOps::resolve_paths(
        &jar_path,
        Some(Path::new("/tmp/krr-unrelated-plantuml-cache")),
    )
    .map_err(|warning| warning.message())?;
    let _ = std::fs::remove_file(&jvm_path);

    assert_eq!(paths.jar_path, jar_path);
    assert_eq!(paths.jvm_path, jvm_path);
    Ok(())
}

fn temp_path(name: &str) -> PathBuf {
    let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "krr-plantuml-resolve-{name}-{}-{id}",
        std::process::id()
    ))
}
