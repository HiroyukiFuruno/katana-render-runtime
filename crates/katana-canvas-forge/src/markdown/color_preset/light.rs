use super::DiagramColorPreset;
use std::sync::OnceLock;

pub struct LightOps;

impl LightOps {
    pub fn get() -> &'static DiagramColorPreset {
        static LIGHT: OnceLock<DiagramColorPreset> = OnceLock::new();
        LIGHT.get_or_init(|| DiagramColorPreset {
            dark_mode: false,
            background: "transparent".into(),
            text: "#333333".into(),
            fill: "#fff2cc".into(),
            stroke: "#d6b656".into(),
            arrow: "#555555".into(),
            drawio_label_color: "#333333".into(),
            mermaid_theme: "default".into(),
            plantuml_class_bg: "#FEFECE".into(),
            plantuml_note_bg: "#FBFB77".into(),
            plantuml_note_text: "#333333".into(),
            syntax_theme_dark: "base16-ocean.dark".into(),
            syntax_theme_light: "InspiredGitHub".into(),
            preview_text: "#333333".into(),
            proportional_font_candidates: DiagramColorPreset::default_proportional_fonts(),
            monospace_font_candidates: DiagramColorPreset::default_monospace_fonts(),
            emoji_font_candidates: DiagramColorPreset::default_emoji_fonts(),
            editor_font_size: DiagramColorPreset::DEFAULT_EDITOR_FONT_SIZE,
        })
    }
}
