mod asset;
mod config;
mod resolve;
mod runtime;
mod theme;
mod theme_catalog;
pub mod types;

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
        PlantUmlJvmRuntimeOps::render_svg(&block.source, &paths, &style).map_or_else(
            |error| DiagramResult::Err {
                source: block.source.clone(),
                error,
            },
            DiagramResult::Ok,
        )
    }

    fn raw(block: &DiagramBlock, warning: String) -> DiagramResult {
        DiagramResult::RawCode {
            source: block.source.clone(),
            warning,
        }
    }
}
