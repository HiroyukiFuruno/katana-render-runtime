use super::super::test_support::{EnvOverride, env_guard};
use super::{PLANTUML_JAR_VERSION, PlantUmlJarAssetOps};
use std::path::{Path, PathBuf};

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

#[test]
fn cache_env_falls_back_to_kdr_when_krr_is_missing() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr = EnvOverride::unset("KRR_PLANTUML_CACHE_DIR");
    let _kdr = EnvOverride::set("KDR_PLANTUML_CACHE_DIR", "/tmp/kdr-cache");

    assert_eq!(
        PlantUmlJarAssetOps::cache_path(None),
        Path::new("/tmp/kdr-cache")
            .join(PLANTUML_JAR_VERSION)
            .join("plantuml.jar")
    );
    Ok(())
}

#[test]
fn asset_helpers_cover_digest_temp_path_and_empty_environment() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr = EnvOverride::set("KRR_PLANTUML_CACHE_DIR", "");
    let _kdr = EnvOverride::set("KDR_PLANTUML_CACHE_DIR", "");

    assert!(PlantUmlJarAssetOps::verify_bytes(b"invalid").is_err());
    assert!(PlantUmlJarAssetOps::digest_file(Path::new("")).is_err());
    assert_eq!(PlantUmlJarAssetOps::hex_lower(&[0x0f, 0xa0]), "0fa0");
    let temporary = PlantUmlJarAssetOps::temp_path(Path::new(""));
    let name = temporary
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("temporary path has no file name")?;
    assert!(name.starts_with("plantuml.jar.tmp-"));
    assert!(PlantUmlJarAssetOps::cache_path(None).ends_with("plantuml.jar"));
    Ok(())
}

#[test]
#[cfg(unix)]
fn temp_path_falls_back_for_non_utf8_file_names() -> Result<(), String> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let path = PathBuf::from(OsString::from_vec(vec![0xff]));
    let temporary = PlantUmlJarAssetOps::temp_path(&path);
    let name = temporary
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("temporary path has no file name")?;

    assert!(name.starts_with("plantuml.jar.tmp-"));
    Ok(())
}
