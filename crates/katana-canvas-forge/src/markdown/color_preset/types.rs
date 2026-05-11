use std::borrow::Cow;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Default)]
pub struct DiagramColorPreset {
    pub dark_mode: bool,
    pub background: Cow<'static, str>,
    pub text: Cow<'static, str>,
    pub fill: Cow<'static, str>,
    pub stroke: Cow<'static, str>,
    pub arrow: Cow<'static, str>,
    pub drawio_label_color: Cow<'static, str>,
    pub mermaid_theme: Cow<'static, str>,
    pub plantuml_class_bg: Cow<'static, str>,
    pub plantuml_note_bg: Cow<'static, str>,
    pub plantuml_note_text: Cow<'static, str>,
    pub syntax_theme_dark: Cow<'static, str>,
    pub syntax_theme_light: Cow<'static, str>,
    pub preview_text: Cow<'static, str>,
    pub proportional_font_candidates: Vec<&'static str>,
    pub monospace_font_candidates: Vec<&'static str>,
    pub emoji_font_candidates: Vec<&'static str>,
    pub editor_font_size: f32,
}

pub static DARK_MODE: AtomicBool = AtomicBool::new(true);
