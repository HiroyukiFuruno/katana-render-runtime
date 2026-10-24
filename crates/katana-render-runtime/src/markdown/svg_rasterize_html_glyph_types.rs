use resvg::usvg;
use skia_safe::Typeface;

#[cfg(all(test, target_os = "macos"))]
use core_graphics::font::CGFont;

#[derive(Clone)]
pub(super) enum CachedTypeface {
    Skia(Typeface),
    #[cfg(all(test, target_os = "macos"))]
    CoreText(CGFont),
}

impl CachedTypeface {
    pub(super) fn as_skia(&self) -> Result<&Typeface, String> {
        match self {
            Self::Skia(typeface) => Ok(typeface),
            #[cfg(all(test, target_os = "macos"))]
            Self::CoreText(_) => Err("CoreText face is not a Skia typeface".to_string()),
        }
    }
}

#[derive(Clone)]
pub(super) struct CachedFace {
    pub(super) typeface: CachedTypeface,
    pub(super) upem: f32,
    pub(super) ascent: f32,
    pub(super) descent: f32,
}

pub(super) struct SpanInput {
    pub(super) color: usvg::Color,
    pub(super) opacity: u8,
    pub(super) font: usvg::fontdb::ID,
    pub(super) face: CachedFace,
    pub(super) font_size: f32,
    pub(super) glyph_ids: Vec<u16>,
}

pub(super) struct FaceMetadata {
    pub(super) upem: f32,
    pub(super) unsupported: bool,
    pub(super) ascent: f32,
    pub(super) descent: f32,
}
