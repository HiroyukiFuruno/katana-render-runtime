use super::cache::MermaidSvgCache;
use super::js_runtime::MermaidJsRuntimeOps;
use super::types::MermaidRenderOps;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::diagram_runtime::DiagramRuntimeMode;
use crate::markdown::runtime_assets::{
    MERMAID_DOWNLOAD_URL, MERMAID_ZENUML_JS_CHECKSUM, MERMAID_ZENUML_JS_VERSION,
};
use crate::markdown::runtime_resource_loader::{
    BundledResource, LoadedResource, RuntimeResourceLoader as runtime_resource_loader,
};
use crate::markdown::{DiagramBlock, DiagramResult};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

#[path = "render_instance.rs"]
mod render_instance;
const MERMAID_RENDER_CACHE_SCHEMA: &str = "mermaid-render-theme-v128-zenuml-mermaid-plugin";
const MERMAID_RUNTIME_BUNDLE_CHECKSUMS: &str =
    include_str!("../diagram_runtime/generated/runtime-bundles.sha256");
const MERMAID_ZENUML_PLUGIN_CACHE_KEY: (&str, &str) =
    (MERMAID_ZENUML_JS_VERSION, MERMAID_ZENUML_JS_CHECKSUM);

impl MermaidRenderOps {
    pub fn render_mermaid_with_runtime_path(
        block: &DiagramBlock,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
    ) -> DiagramResult {
        Self::render_mermaid_with_origin(block, mermaid_js, preset, false)
    }

    pub(crate) fn render_mermaid_with_origin(
        block: &DiagramBlock,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
        bundled: bool,
    ) -> DiagramResult {
        if block.source.trim().is_empty() {
            return DiagramResult::Ok(String::new());
        }

        if !bundled && !mermaid_js.exists() {
            return DiagramResult::NotInstalled {
                kind: "Mermaid".to_string(),
                download_url: MERMAID_DOWNLOAD_URL.to_string(),
                install_path: mermaid_js.to_path_buf(),
            };
        }

        let bundle = match runtime_resource_loader::load(
            mermaid_js,
            bundled.then_some(BundledResource::Mermaid),
        ) {
            Ok(bundle) => bundle,
            Err(error) => {
                return Self::error(block, format!("Failed to read Mermaid.js bundle: {error}"));
            }
        };
        Self::render_mermaid_with_loaded(block, mermaid_js, preset, &bundle)
    }

    pub(crate) fn render_mermaid_with_loaded(
        block: &DiagramBlock,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
        bundle: &LoadedResource,
    ) -> DiagramResult {
        if block.source.trim().is_empty() {
            return DiagramResult::Ok(String::new());
        }
        let mode = DiagramRuntimeMode::current();
        let mut cache_file = Self::cache_file_path(&block.source, mermaid_js, preset, mode);
        if let Some(digest) = bundle.digest() {
            cache_file.set_extension(format!("{digest}.{}", mode.mermaid_cache_extension()));
        }
        if let Err(error) = MermaidSvgCache::ensure_parent(&cache_file) {
            return Self::error(block, error);
        }
        Self::render_svg_with_bundle(block, bundle.text(), preset, &cache_file)
    }

    #[cfg(test)]
    fn render_mermaid_with_cache_file(
        block: &DiagramBlock,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
        cache_file: &Path,
    ) -> DiagramResult {
        if let Err(error) = MermaidSvgCache::ensure_parent(cache_file) {
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

    #[cfg(test)]
    fn render_svg(
        block: &DiagramBlock,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
        cache_file: &Path,
    ) -> DiagramResult {
        match MermaidSvgCache::read(cache_file) {
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

    fn render_svg_with_bundle(
        block: &DiagramBlock,
        bundle: &str,
        preset: &DiagramColorPreset,
        cache_file: &Path,
    ) -> DiagramResult {
        match MermaidSvgCache::read(cache_file) {
            Ok(Some(svg)) => return DiagramResult::Ok(Self::unique_svg_instance(svg)),
            Ok(None) => {}
            Err(error) => return Self::error(block, error),
        }
        match MermaidJsRuntimeOps::render_bundle(&block.source, bundle, preset) {
            Ok(svg) => Self::write_cached_svg(block, cache_file, svg),
            Err(error) => Self::error(block, error),
        }
    }

    fn cache_file_path(
        source: &str,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
        mode: DiagramRuntimeMode,
    ) -> PathBuf {
        Self::cache_file_path_with_zenuml_key(
            source,
            mermaid_js,
            preset,
            mode,
            MERMAID_ZENUML_PLUGIN_CACHE_KEY,
        )
    }

    fn cache_file_path_with_zenuml_key(
        source: &str,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
        mode: DiagramRuntimeMode,
        zenuml_plugin_key: (&str, &str),
    ) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        MERMAID_RENDER_CACHE_SCHEMA.hash(&mut hasher);
        MERMAID_RUNTIME_BUNDLE_CHECKSUMS.hash(&mut hasher);
        zenuml_plugin_key.hash(&mut hasher);
        mermaid_js.hash(&mut hasher);
        mode.mermaid_cache_profile().hash(&mut hasher);
        super::hash_mermaid_source_and_preset(&mut hasher, source, preset);
        preset.dark_mode.hash(&mut hasher);
        std::env::temp_dir()
            .join("katana_mermaid_cache")
            .join(format!(
                "{:016x}.{}",
                hasher.finish(),
                mode.mermaid_cache_extension()
            ))
    }

    fn write_cached_svg(block: &DiagramBlock, cache_file: &Path, svg: String) -> DiagramResult {
        if let Err(error) = MermaidSvgCache::write(cache_file, svg.as_bytes()) {
            return Self::error(block, error);
        }
        DiagramResult::Ok(Self::unique_svg_instance(svg))
    }

    fn error(block: &DiagramBlock, error: String) -> DiagramResult {
        DiagramResult::Err {
            source: block.source.clone(),
            error,
        }
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
