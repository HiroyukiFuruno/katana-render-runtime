pub mod dark;
pub mod light;
pub mod types;

use crate::renderer::{RenderInput, RenderThemeMode, RenderThemeSnapshot};
use std::sync::atomic::Ordering;
pub use types::{DARK_MODE, DiagramColorPreset};

impl DiagramColorPreset {
    pub const DEFAULT_EDITOR_FONT_SIZE: f32 = 14.0;

    pub fn dark() -> &'static Self {
        dark::DarkOps::get()
    }

    pub fn light() -> &'static Self {
        light::LightOps::get()
    }

    pub fn is_dark_mode() -> bool {
        DARK_MODE.load(Ordering::Relaxed)
    }

    pub fn set_dark_mode(is_dark: bool) {
        DARK_MODE.store(is_dark, Ordering::Relaxed);
    }

    pub fn current() -> &'static Self {
        if Self::is_dark_mode() {
            Self::dark()
        } else {
            Self::light()
        }
    }

    pub(crate) fn for_render_input(input: &RenderInput) -> Self {
        input
            .context
            .theme
            .as_ref()
            .map_or_else(|| Self::current().clone(), Self::from_theme_snapshot)
    }

    pub fn from_theme_snapshot(snapshot: &RenderThemeSnapshot) -> Self {
        Self {
            dark_mode: snapshot.mode == RenderThemeMode::Dark,
            background: snapshot.background.clone().into(),
            text: snapshot.text.clone().into(),
            fill: snapshot.fill.clone().into(),
            stroke: snapshot.stroke.clone().into(),
            arrow: snapshot.arrow.clone().into(),
            drawio_label_color: snapshot.drawio_label_color.clone().into(),
            mermaid_theme: snapshot.mermaid_theme.clone().into(),
            plantuml_class_bg: snapshot.plantuml_class_bg.clone().into(),
            plantuml_note_bg: snapshot.plantuml_note_bg.clone().into(),
            plantuml_note_text: snapshot.plantuml_note_text.clone().into(),
            syntax_theme_dark: snapshot.syntax_theme_dark.clone().into(),
            syntax_theme_light: snapshot.syntax_theme_light.clone().into(),
            preview_text: snapshot.preview_text.clone().into(),
            proportional_font_candidates: Self::default_proportional_fonts(),
            monospace_font_candidates: Self::default_monospace_fonts(),
            emoji_font_candidates: Self::default_emoji_fonts(),
            editor_font_size: Self::DEFAULT_EDITOR_FONT_SIZE,
        }
    }

    pub fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
        const HEX_RGB_LEN: usize = 6;
        const HEX_RADIX: u32 = 16;
        const R_END: usize = 2;
        const G_START: usize = 2;
        const G_END: usize = 4;
        const B_START: usize = 4;

        let hex = hex.strip_prefix('#')?;
        if hex.len() != HEX_RGB_LEN {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..R_END], HEX_RADIX).ok()?;
        let g = u8::from_str_radix(&hex[G_START..G_END], HEX_RADIX).ok()?;
        let b = u8::from_str_radix(&hex[B_START..HEX_RGB_LEN], HEX_RADIX).ok()?;
        Some((r, g, b))
    }

    pub fn relative_luminance(hex: &str) -> Option<f64> {
        const CHANNEL_MAX: f64 = 255.0;
        const LUMA_R: f64 = 0.2126;
        const LUMA_G: f64 = 0.7152;
        const LUMA_B: f64 = 0.0722;

        let (r, g, b) = Self::parse_hex_rgb(hex)?;
        let rf = f64::from(r) / CHANNEL_MAX;
        let gf = f64::from(g) / CHANNEL_MAX;
        let bf = f64::from(b) / CHANNEL_MAX;
        Some(LUMA_R * rf + LUMA_G * gf + LUMA_B * bf)
    }

    pub fn default_proportional_fonts() -> Vec<&'static str> {
        vec![
            "/System/Library/Fonts/\u{30d2}\u{30e9}\u{30ae}\u{30ce}\u{89d2}\u{30b4}\u{30b7}\u{30c3}\u{30af} W3.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/AquaKana.ttc",
            "C:/Windows/Fonts/YuGothR.ttc",
            "C:/Windows/Fonts/yugothic.ttf",
            "C:/Windows/Fonts/meiryo.ttc",
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]
    }

    pub fn default_monospace_fonts() -> Vec<&'static str> {
        vec![
            "/System/Library/Fonts/Menlo.ttc",
            "/System/Library/Fonts/SFMono-Regular.otf",
            "/System/Library/Fonts/Monaco.ttf",
            "C:/Windows/Fonts/consola.ttf",
            "C:/Windows/Fonts/cour.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        ]
    }

    pub fn default_emoji_fonts() -> Vec<&'static str> {
        vec![
            "/System/Library/Fonts/Apple Color Emoji.ttc",
            "C:/Windows/Fonts/seguiemj.ttf",
            "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
            "/usr/share/fonts/google-noto-emoji/NotoColorEmoji.ttf",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::DiagramColorPreset;
    use crate::renderer::{
        DiagramKind, RenderConfig, RenderContext, RenderInput, RenderPolicy, RenderThemeMode,
        RenderThemeSnapshot,
    };
    use std::sync::{Mutex, MutexGuard};

    static MODE_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn light_and_dark_presets_keep_katana_theme_names() {
        assert_eq!(DiagramColorPreset::dark().mermaid_theme, "dark");
        assert_eq!(DiagramColorPreset::light().mermaid_theme, "default");
    }

    #[test]
    fn hex_luminance_accepts_six_digit_colors() {
        assert!(DiagramColorPreset::relative_luminance("#ffffff").is_some_and(|it| it > 0.9));
        assert_eq!(DiagramColorPreset::parse_hex_rgb("ffffff"), None);
        assert_eq!(DiagramColorPreset::parse_hex_rgb("#fff"), None);
        assert_eq!(DiagramColorPreset::parse_hex_rgb("#xxffff"), None);
    }

    #[test]
    fn current_tracks_dark_mode_and_default_font_lists_are_populated() {
        let _guard = mode_guard();
        DiagramColorPreset::set_dark_mode(true);
        assert_eq!(DiagramColorPreset::current().mermaid_theme, "dark");
        DiagramColorPreset::set_dark_mode(false);
        assert_eq!(DiagramColorPreset::current().mermaid_theme, "default");
        assert!(!DiagramColorPreset::default_proportional_fonts().is_empty());
        assert!(!DiagramColorPreset::default_monospace_fonts().is_empty());
        assert!(!DiagramColorPreset::default_emoji_fonts().is_empty());
    }

    #[test]
    fn for_render_input_prefers_render_input_theme_over_global_state() {
        let _guard = mode_guard();
        DiagramColorPreset::set_dark_mode(true);
        let preset = DiagramColorPreset::for_render_input(&input(Some(light_snapshot())));

        assert!(!preset.dark_mode);
        assert_eq!(preset.mermaid_theme, "default");
        assert_eq!(preset.text, "#333333");
    }

    #[test]
    fn for_render_input_falls_back_to_global_state() {
        let _guard = mode_guard();
        DiagramColorPreset::set_dark_mode(true);
        let preset = DiagramColorPreset::for_render_input(&input(None));

        assert!(preset.dark_mode);
        assert_eq!(preset.mermaid_theme, "dark");
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

    fn input(theme: Option<RenderThemeSnapshot>) -> RenderInput {
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

    fn light_snapshot() -> RenderThemeSnapshot {
        RenderThemeSnapshot {
            mode: RenderThemeMode::Light,
            background: "transparent".to_string(),
            text: "#333333".to_string(),
            fill: "#fff2cc".to_string(),
            stroke: "#d6b656".to_string(),
            arrow: "#555555".to_string(),
            drawio_label_color: "#333333".to_string(),
            mermaid_theme: "default".to_string(),
            plantuml_class_bg: "#FEFECE".to_string(),
            plantuml_note_bg: "#FBFB77".to_string(),
            plantuml_note_text: "#333333".to_string(),
            syntax_theme_dark: "base16-ocean.dark".to_string(),
            syntax_theme_light: "InspiredGitHub".to_string(),
            preview_text: "#333333".to_string(),
        }
    }

    fn mode_guard() -> MutexGuard<'static, ()> {
        match MODE_LOCK.lock() {
            Ok(guard) => guard,
            Err(error) => error.into_inner(),
        }
    }
}
