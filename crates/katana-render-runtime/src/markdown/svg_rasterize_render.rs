use super::{
    GeneratedClipProvenance, composite, effective_scale, provenance, types::SvgRasterizeError,
};
use resvg::usvg;
use tiny_skia::Pixmap;

pub(super) struct RasterTarget {
    pub(super) display_width: f32,
    pub(super) display_height: f32,
    pub(super) effective_scale: f32,
    pub(super) width: u32,
    pub(super) height: u32,
}

impl RasterTarget {
    pub(super) fn new(size: usvg::Size, scale: f32) -> Self {
        let display_width = size.width();
        let display_height = size.height();
        let effective_scale = effective_scale(display_width, display_height, scale);
        Self {
            display_width,
            display_height,
            effective_scale,
            width: ((display_width * effective_scale).ceil() as u32).max(1),
            height: ((display_height * effective_scale).ceil() as u32).max(1),
        }
    }

    pub(super) fn render(
        &self,
        tree: &usvg::Tree,
        compatible_svg: &str,
        generated: &GeneratedClipProvenance,
    ) -> Result<Pixmap, SvgRasterizeError> {
        let Some(mut pixmap) = Pixmap::new(self.width, self.height) else {
            return Err(SvgRasterizeError::RasterizeFailed(
                "failed to allocate SVG pixmap".to_string(),
            ));
        };
        let transform =
            tiny_skia::Transform::from_scale(self.effective_scale, self.effective_scale);
        let authored_clip_info = if composite::SvgCompositeRenderer::needs_authored_clip_ids(tree) {
            provenance::authored_clip_ids(compatible_svg, generated)?
        } else {
            provenance::AuthoredClipInfo::default()
        };
        composite::SvgCompositeRenderer::render(
            tree,
            transform,
            &mut pixmap.as_mut(),
            &authored_clip_info,
        )
        .map_err(SvgRasterizeError::RasterizeFailed)?;
        Ok(pixmap)
    }
}
