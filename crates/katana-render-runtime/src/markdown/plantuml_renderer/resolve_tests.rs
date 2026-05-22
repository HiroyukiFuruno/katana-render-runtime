use super::super::asset::PlantUmlJarAssetOps;
use super::PlantUmlRuntimePathOps;
use std::path::PathBuf;

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
            && warning.message().contains("KDR_PLANTUML_CACHE_DIR")
            && warning.message().contains("network access")
    ));
}

#[test]
fn api_cache_dir_overrides_default_cache_path() {
    if std::env::var_os("KDR_PLANTUML_JAR").is_some() || std::env::var_os("PLANTUML_JAR").is_some()
    {
        return;
    }
    let default_path = PlantUmlJarAssetOps::cache_path(None);
    let cache_dir = PathBuf::from("/tmp/kdr-api-cache");
    let effective =
        PlantUmlRuntimePathOps::effective_jar_path(&default_path, Some(cache_dir.as_path()));

    assert_eq!(
        effective,
        PlantUmlJarAssetOps::cache_path(Some(cache_dir.as_path()))
    );
}

#[test]
fn missing_libjvm_create_actionable_warning() {
    let result = PlantUmlRuntimePathOps::resolve_jvm_from_candidates(vec![PathBuf::from(
        "target/kdr tests/missing libjvm.dylib",
    )]);

    assert!(matches!(
        result,
        Err(warning) if warning.message().contains("libjvm was not found")
            && warning.message().contains("KDR_PLANTUML_JVM")
            && warning.message().contains("JAVA_HOME")
            && warning.message().contains("target/kdr tests/missing libjvm.dylib")
            && warning.message().contains("install a JDK")
    ));
}

#[test]
fn jar_path_with_spaces_is_reported_without_shell_splitting() {
    let result = PlantUmlRuntimePathOps::resolve_existing_jar(
        "target/kdr tests/missing jar.jar".as_ref(),
        None,
    );

    assert!(matches!(
        result,
        Err(warning) if warning.message().contains("target/kdr tests/missing jar.jar")
    ));
}
