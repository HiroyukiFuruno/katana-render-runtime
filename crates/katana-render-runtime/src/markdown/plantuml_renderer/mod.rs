mod asset;
mod config;
mod resolve;
mod runtime;
mod theme;
mod theme_catalog;
pub mod types;

#[cfg(test)]
mod test_support;

pub use asset::{PLANTUML_DOWNLOAD_URL, PLANTUML_JAR_CHECKSUM, PLANTUML_JAR_VERSION};
pub(crate) use config::PlantUmlRuntimeConfig;
pub(crate) use theme::PlantUmlThemeConfig;
pub use theme_catalog::PlantUmlThemeCatalog;
pub use types::{PlantUmlRendererOps, PlantUmlRuntimeWarning};

use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::{DiagramBlock, DiagramResult};
use resolve::PlantUmlRuntimePathOps;
use runtime::PlantUmlJvmRuntimeOps;
use std::path::{Path, PathBuf};
use theme::PlantUmlThemeOps;

impl PlantUmlRendererOps {
    pub fn default_jar_path() -> PathBuf {
        PlantUmlRuntimePathOps::surface_jar_path()
    }

    pub fn default_jar_path_for_cache_dir(cache_dir: &Path) -> PathBuf {
        PlantUmlRuntimePathOps::surface_jar_path_for_cache_dir(cache_dir)
    }

    pub(crate) fn render_plantuml_with_jar_path(
        block: &DiagramBlock,
        jar_path: &Path,
        preset: &DiagramColorPreset,
        theme_config: &PlantUmlThemeConfig,
        runtime_config: &PlantUmlRuntimeConfig,
    ) -> DiagramResult {
        if block.source.trim().is_empty() {
            return DiagramResult::Ok(String::new());
        }
        let cache_dir = runtime_config.cache_dir();
        let effective_jar_path =
            PlantUmlRuntimePathOps::effective_jar_path(jar_path, cache_dir.as_deref());
        let paths = match PlantUmlRuntimePathOps::resolve_paths(
            &effective_jar_path,
            cache_dir.as_deref(),
        ) {
            Ok(paths) => paths,
            Err(warning) => return Self::raw(block, warning.message()),
        };
        let style = PlantUmlThemeOps::style(preset, theme_config);
        match PlantUmlJvmRuntimeOps::render_svg(&block.source, &paths, &style) {
            Ok(svg) => DiagramResult::Ok(svg),
            Err(error) => DiagramResult::Err {
                source: block.source.clone(),
                error,
            },
        }
    }

    fn raw(block: &DiagramBlock, warning: String) -> DiagramResult {
        DiagramResult::RawCode {
            source: block.source.clone(),
            warning,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::DiagramKind;

    #[test]
    fn render_plantuml_with_jar_path_reports_runtime_errors() -> Result<(), String> {
        let result = render_cached("@startuml\n!error forced test failure\n@enduml")?;

        assert!(matches!(
            result,
            DiagramResult::Err { error, .. } if error.contains("PlantUML render failed")
        ));
        Ok(())
    }

    #[test]
    fn render_plantuml_with_jar_path_returns_runtime_svg() -> Result<(), String> {
        let result = render_cached("@startuml\nAlice -> Bob\n@enduml")?;

        assert!(matches!(
            result,
            DiagramResult::Ok(svg) if svg.contains("<svg")
        ));
        Ok(())
    }

    fn render_cached(source: &str) -> Result<DiagramResult, String> {
        let _guard = asset::PLANTUML_ENV_LOCK.lock().map_err(lock_error)?;
        let jar_path = asset::PlantUmlJarAssetOps::cache_path(None);
        assert!(jar_path.exists(), "PlantUML cache jar is required");
        let block = DiagramBlock {
            kind: DiagramKind::PlantUml,
            source: source.to_string(),
        };
        Ok(PlantUmlRendererOps::render_plantuml_with_jar_path(
            &block,
            &jar_path,
            DiagramColorPreset::light(),
            &PlantUmlThemeConfig::default(),
            &PlantUmlRuntimeConfig::default(),
        ))
    }

    fn lock_error(error: impl ToString) -> String {
        error.to_string()
    }

    #[test]
    fn lock_error_preserves_environment_lock_messages() {
        assert_eq!(
            lock_error("PlantUML environment lock failed"),
            "PlantUML environment lock failed"
        );
    }
}
