use super::js_runtime::MermaidJsRuntimeOps;
use super::types::MermaidRenderOps;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::diagram_runtime::DiagramRuntimeMode;
use crate::markdown::runtime_assets::MERMAID_DOWNLOAD_URL;
use crate::markdown::{DiagramBlock, DiagramResult};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static MERMAID_SVG_RENDER_SEQUENCE: AtomicU64 = AtomicU64::new(1);

impl MermaidRenderOps {
    pub fn render_mermaid_with_runtime_path(
        block: &DiagramBlock,
        mermaid_js: &Path,
    ) -> DiagramResult {
        if block.source.trim().is_empty() {
            return DiagramResult::Ok(String::new());
        }

        if !mermaid_js.exists() {
            return DiagramResult::NotInstalled {
                kind: "Mermaid".to_string(),
                download_url: MERMAID_DOWNLOAD_URL.to_string(),
                install_path: mermaid_js.to_path_buf(),
            };
        }

        let preset = DiagramColorPreset::current();
        let mode = DiagramRuntimeMode::current();
        let cache_file = Self::cache_file_path(&block.source, preset, mode);
        Self::render_mermaid_with_cache_file(block, mermaid_js, preset, &cache_file)
    }

    fn render_mermaid_with_cache_file(
        block: &DiagramBlock,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
        cache_file: &Path,
    ) -> DiagramResult {
        if let Err(error) = Self::ensure_cache_parent(cache_file) {
            return DiagramResult::Err {
                source: block.source.clone(),
                error,
            };
        }

        Self::render_svg(block, mermaid_js, preset, cache_file)
    }

    pub fn cache_profile() -> &'static str {
        DiagramRuntimeMode::current().mermaid_cache_profile()
    }

    fn render_svg(
        block: &DiagramBlock,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
        cache_file: &Path,
    ) -> DiagramResult {
        match Self::read_cached_svg(cache_file) {
            Ok(Some(svg)) => return DiagramResult::Ok(Self::unique_svg_instance(svg)),
            Ok(None) => {}
            Err(error) => {
                return Self::error(block, error);
            }
        }
        match MermaidJsRuntimeOps::render(&block.source, mermaid_js, preset) {
            Ok(svg) => Self::write_cached_svg(block, cache_file, svg),
            Err(error) => Self::error(block, error),
        }
    }

    fn cache_file_path(
        source: &str,
        preset: &DiagramColorPreset,
        mode: DiagramRuntimeMode,
    ) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        "mermaid-render-theme-v120-ja-parts-layout-fixes".hash(&mut hasher);
        mode.mermaid_cache_profile().hash(&mut hasher);
        source.hash(&mut hasher);
        preset.mermaid_theme.hash(&mut hasher);
        preset.background.hash(&mut hasher);
        preset.text.hash(&mut hasher);
        preset.fill.hash(&mut hasher);
        preset.stroke.hash(&mut hasher);
        preset.arrow.hash(&mut hasher);
        std::env::temp_dir()
            .join("katana_mermaid_cache")
            .join(format!(
                "{:016x}.{}",
                hasher.finish(),
                mode.mermaid_cache_extension()
            ))
    }

    fn ensure_cache_parent(cache_file: &Path) -> Result<(), String> {
        let Some(parent) = cache_file.parent() else {
            return Err("Mermaid cache path has no parent directory".to_string());
        };
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())
    }

    fn read_cached_svg(cache_file: &Path) -> Result<Option<String>, String> {
        match std::fs::read_to_string(cache_file) {
            Ok(svg) => Ok(Some(svg)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn write_cached_svg(block: &DiagramBlock, cache_file: &Path, svg: String) -> DiagramResult {
        if let Err(error) = std::fs::write(cache_file, &svg) {
            return Self::error(block, error.to_string());
        }
        DiagramResult::Ok(Self::unique_svg_instance(svg))
    }

    fn error(block: &DiagramBlock, error: String) -> DiagramResult {
        DiagramResult::Err {
            source: block.source.clone(),
            error,
        }
    }

    fn unique_svg_instance(svg: String) -> String {
        let Some(root_id) = Self::root_svg_id(&svg) else {
            return svg;
        };
        let sequence = MERMAID_SVG_RENDER_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let unique_id = format!("{root_id}-{sequence:016x}");
        svg.replace(&root_id, &unique_id)
    }

    fn root_svg_id(svg: &str) -> Option<String> {
        let svg_start = svg.find("<svg")?;
        let open_end = svg_start + svg[svg_start..].find('>')?;
        let marker = r#"id="katana-mermaid-svg-"#;
        let start = svg_start + svg[svg_start..open_end].find(marker)? + r#"id=""#.len();
        let end = start + svg[start..open_end].find('"')?;
        Some(svg[start..end].to_string())
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
