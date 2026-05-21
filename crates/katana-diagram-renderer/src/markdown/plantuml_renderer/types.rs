use super::asset::PLANTUML_DOWNLOAD_URL;

pub struct PlantUmlRendererOps;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlantUmlRuntimeWarning {
    pub code: &'static str,
    pub reason: String,
    pub checked_env: Vec<&'static str>,
    pub checked_paths: Vec<String>,
}

impl PlantUmlRuntimeWarning {
    pub fn new(
        reason: impl Into<String>,
        checked_env: Vec<&'static str>,
        checked_paths: Vec<String>,
    ) -> Self {
        Self {
            code: "plantuml-runtime-unavailable",
            reason: reason.into(),
            checked_env,
            checked_paths,
        }
    }

    pub fn message(&self) -> String {
        format!(
            "warning[{}]: {}. checked_env={}. checked_paths={}. action={}",
            self.code,
            self.reason,
            self.checked_env.join(","),
            self.checked_paths.join(","),
            Self::install_hint()
        )
    }

    fn install_hint() -> String {
        format!(
            "install a JDK with libjvm and set KDR_PLANTUML_JVM or JAVA_HOME; KDR downloads the pinned PlantUML JAR from {PLANTUML_DOWNLOAD_URL} into the cache on first use, so keep network access available, set KDR_PLANTUML_CACHE_DIR or API plantuml_cache_dir to a writable cache directory, or set KDR_PLANTUML_JAR to a readable plantuml.jar"
        )
    }
}
