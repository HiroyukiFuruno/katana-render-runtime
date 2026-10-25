use resvg::usvg::{
    self,
    fontdb::{Database, ID},
};

pub(in crate::markdown::svg_rasterize) fn loaded_html_font(
    font: &usvg::Font,
    database: &Database,
) -> Option<ID> {
    #[cfg(target_os = "macos")]
    if let Some(selection) = super::select_macos_html_font_override(font, database) {
        return selection;
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    if requests_system_font(font) {
        return None;
    }
    super::select_font_case_insensitively(font, database)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(super) fn requests_system_font(font: &usvg::Font) -> bool {
    font.families().iter().any(|family| match family {
        usvg::FontFamily::Named(name) => matches!(
            name.to_ascii_lowercase().as_str(),
            "system-ui" | "-apple-system" | "blinkmacsystemfont"
        ),
        _ => false,
    })
}
