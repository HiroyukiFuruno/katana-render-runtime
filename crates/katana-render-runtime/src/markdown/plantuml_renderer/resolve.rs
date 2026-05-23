use super::asset::PlantUmlJarAssetOps;
use super::types::PlantUmlRuntimeWarning;
use std::path::{Path, PathBuf};

const KRR_PLANTUML_JAR_ENV: &str = "KRR_PLANTUML_JAR";
const KDR_PLANTUML_JAR_ENV: &str = "KDR_PLANTUML_JAR";
const PLANTUML_JAR_ENV: &str = "PLANTUML_JAR";
const KRR_PLANTUML_JVM_ENV: &str = "KRR_PLANTUML_JVM";
const KDR_PLANTUML_JVM_ENV: &str = "KDR_PLANTUML_JVM";
const JAVA_HOME_ENV: &str = "JAVA_HOME";
const KRR_PLANTUML_CACHE_ENV: &str = "KRR_PLANTUML_CACHE_DIR";
const KDR_PLANTUML_CACHE_ENV: &str = "KDR_PLANTUML_CACHE_DIR";

pub(crate) struct PlantUmlRuntimePaths {
    pub(crate) jvm_path: PathBuf,
    pub(crate) jar_path: PathBuf,
}

pub(crate) struct PlantUmlRuntimePathOps;

impl PlantUmlRuntimePathOps {
    pub(crate) fn surface_jar_path() -> PathBuf {
        Self::surface_jar_env_path().unwrap_or_else(|| PlantUmlJarAssetOps::cache_path(None))
    }

    pub(crate) fn surface_jar_path_for_cache_dir(cache_dir: &Path) -> PathBuf {
        Self::surface_jar_env_path()
            .unwrap_or_else(|| PlantUmlJarAssetOps::cache_path(Some(cache_dir)))
    }

    pub(crate) fn effective_jar_path(jar_path: &Path, cache_dir: Option<&Path>) -> PathBuf {
        if cache_dir.is_some() && Self::can_override_cache_path(jar_path) {
            return PlantUmlJarAssetOps::cache_path(cache_dir);
        }
        jar_path.to_path_buf()
    }

    pub(crate) fn resolve_paths(
        jar_path: &Path,
        cache_dir: Option<&Path>,
    ) -> Result<PlantUmlRuntimePaths, PlantUmlRuntimeWarning> {
        let jar_path = Self::resolve_existing_jar(jar_path, cache_dir)?;
        let jvm_path = Self::resolve_existing_jvm()?;
        Ok(PlantUmlRuntimePaths { jvm_path, jar_path })
    }

    fn resolve_existing_jar(
        jar_path: &Path,
        cache_dir: Option<&Path>,
    ) -> Result<PathBuf, PlantUmlRuntimeWarning> {
        if Self::is_cache_jar_path(jar_path, cache_dir) {
            return PlantUmlJarAssetOps::prepare_cache_jar(cache_dir).map_err(Self::jar_warning);
        }
        if !jar_path.exists() {
            let candidates = vec![jar_path.to_path_buf()];
            return Err(PlantUmlRuntimeWarning::new(
                "plantuml.jar was not found",
                Self::jar_env_names(),
                Self::display_paths(&candidates),
            ));
        }
        PlantUmlJarAssetOps::verify_jar(jar_path).map_err(Self::jar_warning)?;
        Ok(jar_path.to_path_buf())
    }

    fn can_override_cache_path(jar_path: &Path) -> bool {
        Self::surface_jar_env_path().is_none() && Self::is_cache_jar_path(jar_path, None)
    }

    fn is_cache_jar_path(jar_path: &Path, cache_dir: Option<&Path>) -> bool {
        jar_path == PlantUmlJarAssetOps::cache_path(cache_dir)
    }

    fn jar_warning(reason: String) -> PlantUmlRuntimeWarning {
        PlantUmlRuntimeWarning::new(reason, Self::jar_env_names(), Vec::new())
    }

    fn jar_env_names() -> Vec<&'static str> {
        vec![
            KRR_PLANTUML_JAR_ENV,
            KDR_PLANTUML_JAR_ENV,
            PLANTUML_JAR_ENV,
            KRR_PLANTUML_CACHE_ENV,
            KDR_PLANTUML_CACHE_ENV,
        ]
    }

    fn resolve_existing_jvm() -> Result<PathBuf, PlantUmlRuntimeWarning> {
        let candidates = Self::jvm_candidates();
        Self::resolve_jvm_from_candidates(candidates)
    }

    fn resolve_jvm_from_candidates(
        candidates: Vec<PathBuf>,
    ) -> Result<PathBuf, PlantUmlRuntimeWarning> {
        Self::first_existing(candidates.clone()).ok_or_else(|| {
            PlantUmlRuntimeWarning::new(
                "libjvm was not found",
                vec![KRR_PLANTUML_JVM_ENV, KDR_PLANTUML_JVM_ENV, JAVA_HOME_ENV],
                Self::display_paths(&candidates),
            )
        })
    }

    fn first_existing(candidates: Vec<PathBuf>) -> Option<PathBuf> {
        candidates.into_iter().find(|it| it.exists())
    }

    fn jvm_candidates() -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        Self::push_env_path(&mut candidates, KRR_PLANTUML_JVM_ENV);
        Self::push_env_path(&mut candidates, KDR_PLANTUML_JVM_ENV);
        if let Some(java_home) = Self::env_path(JAVA_HOME_ENV) {
            candidates.extend(Self::java_home_jvm_candidates(&java_home));
        }
        candidates.extend(Self::platform_jvm_candidates());
        candidates
    }

    fn java_home_jvm_candidates(java_home: &Path) -> Vec<PathBuf> {
        vec![
            java_home
                .join("lib")
                .join("server")
                .join(Self::libjvm_name()),
            java_home
                .join("jre")
                .join("lib")
                .join("server")
                .join(Self::libjvm_name()),
            java_home
                .join("bin")
                .join("server")
                .join(Self::libjvm_name()),
        ]
    }

    fn platform_jvm_candidates() -> Vec<PathBuf> {
        if cfg!(target_os = "macos") {
            return vec![
                "/opt/homebrew/opt/openjdk/libexec/openjdk.jdk/Contents/Home/lib/server/libjvm.dylib".into(),
                "/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home/lib/server/libjvm.dylib".into(),
                "/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home/lib/server/libjvm.dylib".into(),
            ];
        }
        if cfg!(target_os = "linux") {
            return vec![
                "/usr/lib/jvm/default-java/lib/server/libjvm.so".into(),
                "/usr/lib/jvm/default/lib/server/libjvm.so".into(),
                "/usr/lib/jvm/java-21-openjdk-amd64/lib/server/libjvm.so".into(),
                "/usr/lib/jvm/java-17-openjdk-amd64/lib/server/libjvm.so".into(),
            ];
        }
        Vec::new()
    }

    fn libjvm_name() -> &'static str {
        if cfg!(target_os = "windows") {
            "jvm.dll"
        } else if cfg!(target_os = "macos") {
            "libjvm.dylib"
        } else {
            "libjvm.so"
        }
    }

    fn push_env_path(candidates: &mut Vec<PathBuf>, name: &'static str) {
        if let Some(path) = Self::env_path(name) {
            candidates.push(path);
        }
    }

    fn surface_jar_env_path() -> Option<PathBuf> {
        [KRR_PLANTUML_JAR_ENV, KDR_PLANTUML_JAR_ENV, PLANTUML_JAR_ENV]
            .into_iter()
            .find_map(Self::env_path)
    }

    fn env_path(name: &'static str) -> Option<PathBuf> {
        let value = std::env::var_os(name)?;
        (!value.is_empty()).then(|| PathBuf::from(value))
    }

    fn display_paths(paths: &[PathBuf]) -> Vec<String> {
        paths.iter().map(|it| it.display().to_string()).collect()
    }
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
