use super::DiagramColorPreset;
use std::sync::OnceLock;

pub struct DarkOps;

impl DarkOps {
    pub fn get() -> &'static DiagramColorPreset {
        static DARK: OnceLock<DiagramColorPreset> = OnceLock::new();
        DARK.get_or_init(|| DiagramColorPreset {
            dark_mode: true,
            background: "transparent".into(),
            text: "#E0E0E0".into(),
            fill: "#2D2D2D".into(),
            stroke: "#888888".into(),
            arrow: "#AAAAAA".into(),
            drawio_label_color: "#1A1A1A".into(),
            mermaid_theme: "dark".into(),
            plantuml_class_bg: "#2D2D2D".into(),
            plantuml_note_bg: "#3A3A3A".into(),
            plantuml_note_text: "#E0E0E0".into(),
            syntax_theme_dark: "base16-ocean.dark".into(),
            syntax_theme_light: "base16-ocean.light".into(),
            preview_text: "#E0E0E0".into(),
            proportional_font_candidates: DiagramColorPreset::default_proportional_fonts(),
            monospace_font_candidates: DiagramColorPreset::default_monospace_fonts(),
            emoji_font_candidates: DiagramColorPreset::default_emoji_fonts(),
            editor_font_size: DiagramColorPreset::DEFAULT_EDITOR_FONT_SIZE,
        })
    }
}
