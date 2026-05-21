use super::api::RenderInput;
use crate::markdown::color_preset::DiagramColorPreset;
use serde_json::Value;
use std::hash::{Hash, Hasher};

pub(super) struct CacheFingerprintOps;

impl CacheFingerprintOps {
    pub(super) fn render(
        input: &RenderInput,
        runtime_version: &str,
        runtime_checksum: &str,
    ) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        input.kind.hash(&mut hasher);
        input.source.hash(&mut hasher);
        Self::rendering_vendor_config(&input.config.vendor_config).hash(&mut hasher);
        input.context.theme_fingerprint.hash(&mut hasher);
        Self::hash_effective_theme(&mut hasher, input);
        runtime_version.hash(&mut hasher);
        runtime_checksum.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    fn hash_effective_theme(hasher: &mut impl Hasher, input: &RenderInput) {
        let preset = DiagramColorPreset::for_render_input(input);
        preset.dark_mode.hash(hasher);
        preset.background.hash(hasher);
        preset.text.hash(hasher);
        preset.fill.hash(hasher);
        preset.stroke.hash(hasher);
        preset.arrow.hash(hasher);
        preset.drawio_label_color.hash(hasher);
        preset.mermaid_theme.hash(hasher);
        preset.plantuml_class_bg.hash(hasher);
        preset.plantuml_note_bg.hash(hasher);
        preset.plantuml_note_text.hash(hasher);
        preset.syntax_theme_dark.hash(hasher);
        preset.syntax_theme_light.hash(hasher);
        preset.preview_text.hash(hasher);
        preset.proportional_font_candidates.hash(hasher);
        preset.monospace_font_candidates.hash(hasher);
        preset.emoji_font_candidates.hash(hasher);
        preset.editor_font_size.to_bits().hash(hasher);
    }

    fn rendering_vendor_config(value: &Value) -> String {
        let Value::Object(map) = value else {
            return value.to_string();
        };
        let mut rendering_map = map.clone();
        rendering_map.remove("plantuml_cache_dir");
        rendering_map.remove("plantumlCacheDir");
        if rendering_map.is_empty() {
            return Value::Null.to_string();
        }
        Value::Object(rendering_map).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::CacheFingerprintOps;
    use crate::markdown::color_preset::DiagramColorPreset;
    use crate::renderer::api::{
        DiagramKind, RenderConfig, RenderContext, RenderInput, RenderPolicy, RenderThemeMode,
        RenderThemeSnapshot,
    };
    use std::sync::{Mutex, MutexGuard};

    static MODE_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn render_fingerprint_changes_with_theme_context() {
        let default = CacheFingerprintOps::render(&input(None), "runtime", "checksum");
        let themed = CacheFingerprintOps::render(&input(Some("theme-a")), "runtime", "checksum");

        assert_ne!(default, themed);
    }

    #[test]
    fn render_fingerprint_changes_with_current_theme() {
        let _guard = mode_guard();
        let original = DiagramColorPreset::is_dark_mode();
        DiagramColorPreset::set_dark_mode(false);
        let light = CacheFingerprintOps::render(&input(None), "runtime", "checksum");
        DiagramColorPreset::set_dark_mode(true);
        let dark = CacheFingerprintOps::render(&input(None), "runtime", "checksum");
        DiagramColorPreset::set_dark_mode(original);

        assert_ne!(light, dark);
    }

    #[test]
    fn render_fingerprint_changes_with_render_input_theme() {
        let light = CacheFingerprintOps::render(
            &input_with_theme(Some(theme_snapshot(RenderThemeMode::Light))),
            "runtime",
            "checksum",
        );
        let dark = CacheFingerprintOps::render(
            &input_with_theme(Some(theme_snapshot(RenderThemeMode::Dark))),
            "runtime",
            "checksum",
        );

        assert_ne!(light, dark);
    }

    #[test]
    fn render_input_theme_ignores_global_state_for_fingerprint() {
        let _guard = mode_guard();
        DiagramColorPreset::set_dark_mode(false);
        let light_global = CacheFingerprintOps::render(
            &input_with_theme(Some(theme_snapshot(RenderThemeMode::Light))),
            "runtime",
            "checksum",
        );
        DiagramColorPreset::set_dark_mode(true);
        let dark_global = CacheFingerprintOps::render(
            &input_with_theme(Some(theme_snapshot(RenderThemeMode::Light))),
            "runtime",
            "checksum",
        );

        assert_eq!(light_global, dark_global);
    }

    #[test]
    fn mode_guard_accepts_poisoned_lock() {
        let poison = std::panic::catch_unwind(|| {
            let _guard = mode_guard();
            std::panic::resume_unwind(Box::new("poison mode guard"));
        });

        assert!(poison.is_err());
        let _guard = mode_guard();
    }

    #[test]
    fn render_fingerprint_changes_with_runtime_checksum() {
        let checksum_a = CacheFingerprintOps::render(&input(None), "runtime", "checksum-a");
        let checksum_b = CacheFingerprintOps::render(&input(None), "runtime", "checksum-b");

        assert_ne!(checksum_a, checksum_b);
    }

    #[test]
    fn render_fingerprint_changes_with_vendor_config() {
        let default =
            CacheFingerprintOps::render(&input_with_vendor_config(None), "runtime", "checksum");
        let themed = CacheFingerprintOps::render(
            &input_with_vendor_config(Some("cyborg")),
            "runtime",
            "checksum",
        );

        assert_ne!(default, themed);
    }

    #[test]
    fn render_fingerprint_ignores_plantuml_cache_dir() {
        let _guard = mode_guard();
        let default =
            CacheFingerprintOps::render(&input_with_vendor_config(None), "runtime", "checksum");
        let cache_dir = CacheFingerprintOps::render(
            &input_with_plantuml_cache_dir("/tmp/kdr-plantuml-cache"),
            "runtime",
            "checksum",
        );

        assert_eq!(default, cache_dir);
    }

    fn input(theme_fingerprint: Option<&str>) -> RenderInput {
        RenderInput {
            kind: DiagramKind::Mermaid,
            source: "graph TD; A-->B".to_string(),
            config: RenderConfig::default(),
            policy: RenderPolicy::default(),
            context: RenderContext {
                theme_fingerprint: theme_fingerprint.map(ToString::to_string),
                document_id: None,
                theme: None,
            },
        }
    }

    fn input_with_theme(theme: Option<RenderThemeSnapshot>) -> RenderInput {
        RenderInput {
            kind: DiagramKind::Mermaid,
            source: "graph TD; A-->B".to_string(),
            config: RenderConfig::default(),
            policy: RenderPolicy::default(),
            context: RenderContext {
                theme_fingerprint: None,
                document_id: None,
                theme,
            },
        }
    }

    fn input_with_vendor_config(theme: Option<&str>) -> RenderInput {
        let vendor_config = theme.map_or(serde_json::Value::Null, |theme| {
            serde_json::json!({
                "plantuml_theme": theme,
            })
        });
        RenderInput {
            kind: DiagramKind::PlantUml,
            source: "@startuml\nclass A\n@enduml".to_string(),
            config: RenderConfig { vendor_config },
            policy: RenderPolicy::default(),
            context: RenderContext {
                theme_fingerprint: None,
                document_id: None,
                theme: None,
            },
        }
    }

    fn input_with_plantuml_cache_dir(cache_dir: &str) -> RenderInput {
        RenderInput {
            kind: DiagramKind::PlantUml,
            source: "@startuml\nclass A\n@enduml".to_string(),
            config: RenderConfig {
                vendor_config: serde_json::json!({
                    "plantuml_cache_dir": cache_dir,
                }),
            },
            policy: RenderPolicy::default(),
            context: RenderContext {
                theme_fingerprint: None,
                document_id: None,
                theme: None,
            },
        }
    }

    fn theme_snapshot(mode: RenderThemeMode) -> RenderThemeSnapshot {
        let preset = match mode {
            RenderThemeMode::Light => DiagramColorPreset::light(),
            RenderThemeMode::Dark => DiagramColorPreset::dark(),
        };
        RenderThemeSnapshot {
            mode,
            background: preset.background.to_string(),
            text: preset.text.to_string(),
            fill: preset.fill.to_string(),
            stroke: preset.stroke.to_string(),
            arrow: preset.arrow.to_string(),
            drawio_label_color: preset.drawio_label_color.to_string(),
            mermaid_theme: preset.mermaid_theme.to_string(),
            plantuml_class_bg: preset.plantuml_class_bg.to_string(),
            plantuml_note_bg: preset.plantuml_note_bg.to_string(),
            plantuml_note_text: preset.plantuml_note_text.to_string(),
            syntax_theme_dark: preset.syntax_theme_dark.to_string(),
            syntax_theme_light: preset.syntax_theme_light.to_string(),
            preview_text: preset.preview_text.to_string(),
        }
    }

    fn mode_guard() -> MutexGuard<'static, ()> {
        match MODE_LOCK.lock() {
            Ok(guard) => guard,
            Err(error) => error.into_inner(),
        }
    }
}
