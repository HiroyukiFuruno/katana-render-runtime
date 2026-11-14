use crate::commands::{DiagramInputFormat, DiagramSourceOptions, ThemeModeArg};
use katana_render_runtime::{DiagramKind, RenderConfig, RenderContext, RenderInput, RenderPolicy};
use std::path::PathBuf;

pub(super) struct DiagramRenderRequest {
    pub(super) input_path: PathBuf,
    pub(super) output_path: Option<PathBuf>,
    pub(super) runtime: Option<PathBuf>,
    pub(super) theme: Option<String>,
    pub(super) theme_from: Option<String>,
    pub(super) theme_mode: Option<ThemeModeArg>,
    pub(super) cache_dir: Option<PathBuf>,
    pub(super) source_options: DiagramSourceOptions,
}

impl DiagramRenderRequest {
    pub(super) fn validate_input_options(&self, kind: DiagramKind) -> anyhow::Result<()> {
        if kind != DiagramKind::Mermaid
            && (self.source_options.input_format != DiagramInputFormat::Auto
                || self.source_options.render_attestation.is_some()
                || self.source_options.attempt_token.is_some())
        {
            anyhow::bail!("--input-format and render attestation are supported only for mermaid");
        }
        if self.source_options.render_attestation.is_none()
            && self.source_options.attempt_token.is_some()
        {
            anyhow::bail!("--attempt-token requires --render-attestation");
        }
        Ok(())
    }

    pub(super) fn vendor_config(&self, kind: DiagramKind) -> anyhow::Result<serde_json::Value> {
        RenderInputFactory::vendor_config(
            kind,
            self.theme.clone(),
            self.theme_from.clone(),
            self.theme_mode,
            self.cache_dir.clone(),
        )
    }
}

pub(super) struct RenderInputFactory;

impl RenderInputFactory {
    pub(super) fn validate_runtime_options(
        kind: DiagramKind,
        runtime: Option<&PathBuf>,
        cache_dir: Option<&PathBuf>,
    ) -> anyhow::Result<()> {
        if kind == DiagramKind::PlantUml && runtime.is_some() && cache_dir.is_some() {
            anyhow::bail!("--runtime and --cache-dir cannot be used together for plantuml");
        }
        Ok(())
    }

    pub(super) fn create(
        kind: DiagramKind,
        source: String,
        vendor_config: serde_json::Value,
    ) -> RenderInput {
        RenderInput {
            kind,
            source,
            config: RenderConfig { vendor_config },
            policy: RenderPolicy::default(),
            context: RenderContext::default(),
        }
    }

    pub(super) fn vendor_config(
        kind: DiagramKind,
        theme: Option<String>,
        theme_from: Option<String>,
        theme_mode: Option<ThemeModeArg>,
        cache_dir: Option<PathBuf>,
    ) -> anyhow::Result<serde_json::Value> {
        if Self::has_no_vendor_config(&theme, &theme_from, &theme_mode, &cache_dir) {
            return Ok(serde_json::Value::Null);
        }
        Self::ensure_plantuml_vendor_config(kind)?;
        Ok(Self::plantuml_vendor_config(
            theme, theme_from, theme_mode, cache_dir,
        ))
    }

    fn has_no_vendor_config(
        theme: &Option<String>,
        theme_from: &Option<String>,
        theme_mode: &Option<ThemeModeArg>,
        cache_dir: &Option<PathBuf>,
    ) -> bool {
        theme.is_none() && theme_from.is_none() && theme_mode.is_none() && cache_dir.is_none()
    }

    fn ensure_plantuml_vendor_config(kind: DiagramKind) -> anyhow::Result<()> {
        if kind == DiagramKind::PlantUml {
            return Ok(());
        }
        anyhow::bail!(
            "--theme, --theme-from, --theme-mode, and --cache-dir are currently supported only for plantuml"
        );
    }

    fn plantuml_vendor_config(
        theme: Option<String>,
        theme_from: Option<String>,
        theme_mode: Option<ThemeModeArg>,
        cache_dir: Option<PathBuf>,
    ) -> serde_json::Value {
        serde_json::json!({
            "plantuml_theme": Self::string_or_empty(theme),
            "plantuml_theme_from": Self::string_or_empty(theme_from),
            "plantuml_theme_mode": theme_mode.map_or("", ThemeModeArg::as_str),
            "plantuml_cache_dir": cache_dir
                .map_or_else(String::new, |it| it.display().to_string()),
        })
    }

    fn string_or_empty(value: Option<String>) -> String {
        value.into_iter().collect()
    }
}
