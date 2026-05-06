use super::api::RenderInput;
use crate::markdown::color_preset::DiagramColorPreset;
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
        input.context.theme_fingerprint.hash(&mut hasher);
        Self::hash_current_theme(&mut hasher);
        runtime_version.hash(&mut hasher);
        runtime_checksum.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    fn hash_current_theme(hasher: &mut impl Hasher) {
        let preset = DiagramColorPreset::current();
        DiagramColorPreset::is_dark_mode().hash(hasher);
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
}

#[cfg(test)]
mod tests {
    use super::CacheFingerprintOps;
    use crate::markdown::color_preset::DiagramColorPreset;
    use crate::renderer::api::{
        DiagramKind, RenderConfig, RenderContext, RenderInput, RenderPolicy,
    };

    #[test]
    fn render_fingerprint_changes_with_theme_context() {
        let default = CacheFingerprintOps::render(&input(None), "runtime", "checksum");
        let themed = CacheFingerprintOps::render(&input(Some("theme-a")), "runtime", "checksum");

        assert_ne!(default, themed);
    }

    #[test]
    fn render_fingerprint_changes_with_current_theme() {
        let original = DiagramColorPreset::is_dark_mode();
        DiagramColorPreset::set_dark_mode(false);
        let light = CacheFingerprintOps::render(&input(None), "runtime", "checksum");
        DiagramColorPreset::set_dark_mode(true);
        let dark = CacheFingerprintOps::render(&input(None), "runtime", "checksum");
        DiagramColorPreset::set_dark_mode(original);

        assert_ne!(light, dark);
    }

    #[test]
    fn render_fingerprint_changes_with_runtime_checksum() {
        let checksum_a = CacheFingerprintOps::render(&input(None), "runtime", "checksum-a");
        let checksum_b = CacheFingerprintOps::render(&input(None), "runtime", "checksum-b");

        assert_ne!(checksum_a, checksum_b);
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
            },
        }
    }
}
