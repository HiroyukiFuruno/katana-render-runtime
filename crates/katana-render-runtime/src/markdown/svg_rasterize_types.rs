pub struct SvgRasterizeOps;

#[derive(Debug, Clone)]
pub struct RasterizedSvg {
    pub width: u32,
    pub height: u32,
    pub display_width: f32,
    pub display_height: f32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum SvgRasterizeError {
    #[error("Failed to parse SVG: {0}")]
    ParseFailed(String),
    #[error("Failed to rasterize SVG: {0}")]
    RasterizeFailed(String),
}

impl SvgRasterizeError {
    pub(super) fn parse_failed(error: impl ToString) -> Self {
        Self::ParseFailed(error.to_string())
    }
}
