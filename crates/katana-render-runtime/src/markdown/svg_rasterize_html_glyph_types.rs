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
    pub(super) x_height: Option<f32>,
}

pub(super) struct SpanInput {
    pub(super) color: usvg::Color,
    pub(super) opacity: u8,
    pub(super) font: usvg::fontdb::ID,
    pub(super) face: CachedFace,
    pub(super) variable_instance: Option<std::rc::Rc<super::instances::VariableInstance>>,
    pub(super) font_size: f32,
    pub(super) glyph_ids: Vec<u16>,
}

pub(super) struct FaceMetadata {
    pub(super) upem: f32,
    pub(super) unsupported: bool,
    pub(super) ascent: f32,
    pub(super) descent: f32,
    pub(super) x_height: Option<f32>,
}

impl CachedFace {
    pub(super) fn metadata(&self) -> FaceMetadata {
        FaceMetadata {
            upem: self.upem,
            unsupported: false,
            ascent: self.ascent,
            descent: self.descent,
            x_height: self.x_height,
        }
    }
}
