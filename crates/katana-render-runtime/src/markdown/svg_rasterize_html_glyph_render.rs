use super::super::composite::TextRasterizer;
use super::{HtmlGlyphRasterizer, Prepared, PreparedSpan, surface};
use resvg::usvg;
use skia_safe::{AlphaType, Borrows, Canvas, ColorType, ImageInfo, Surface, surfaces};
use tiny_skia::{PixmapMut, Transform};

impl TextRasterizer for HtmlGlyphRasterizer<'_> {
    fn render(
        &self,
        text: &usvg::Text,
        transform: Transform,
        pixmap: &mut PixmapMut<'_>,
    ) -> Result<bool, String> {
        if self.resvg_text_ids.contains(text.id()) {
            return Ok(false);
        }
        let prepared = match self.prepare(text)? {
            Prepared::Unsupported => return Ok(false),
            Prepared::Ready(value) => value,
        };
        /* WHY: usvgは字形も装飾もないTextを保持しない。装飾だけの場合はprepareで既存経路へ戻る。 */
        let full_transform = transform.pre_concat(text.flattened().transform());
        self.render_prepared(prepared, text, full_transform, pixmap)
    }
}

impl HtmlGlyphRasterizer<'_> {
    pub(super) fn render_prepared(
        &self,
        prepared: Vec<PreparedSpan>,
        text: &usvg::Text,
        full_transform: Transform,
        pixmap: &mut PixmapMut<'_>,
    ) -> Result<bool, String> {
        let shifts = prepared
            .iter()
            .fold((0.0_f32, 0.0_f32), |(min, max), span| {
                (min.min(span.baseline_shift), max.max(span.baseline_shift))
            });
        let Some(crop) = surface::text_crop(
            text,
            full_transform,
            pixmap.width(),
            pixmap.height(),
            shifts,
            prepared.iter().any(|span| span.effects.is_some()),
        ) else {
            return Ok(true);
        };
        self.draw_prepared(prepared, full_transform, crop, pixmap)?;
        Ok(true)
    }

    pub(super) fn draw_prepared(
        &self,
        prepared: Vec<PreparedSpan>,
        full_transform: Transform,
        crop: surface::Crop,
        pixmap: &mut PixmapMut<'_>,
    ) -> Result<(), String> {
        self.draw_skia_prepared(prepared, full_transform, &crop, pixmap)
    }

    fn draw_skia_prepared(
        &self,
        prepared: Vec<PreparedSpan>,
        full_transform: Transform,
        crop: &surface::Crop,
        pixmap: &mut PixmapMut<'_>,
    ) -> Result<(), String> {
        /* WHY: BlinkのSVG文字と同様に、等方倍率をfont sizeへ移してstrike選択を揃える。 */
        let (raster_scale, canvas_transform) = skia_transforms(full_transform);
        let mut skia_surface = wrap_pixmap(pixmap)?;
        /* WHY: 旧crop surfaceのdevice境界を、変換設定前の整数clipで維持する。 */
        clip_crop(skia_surface.canvas(), crop);
        let matrix = surface::skia_matrix(canvas_transform, 0.0, 0.0);
        let matrix = skia_safe::M44::from(&matrix);
        skia_surface.canvas().set_matrix(&matrix);
        self.draw_spans(&mut skia_surface, prepared, raster_scale)?;
        drop(skia_surface);
        Ok(())
    }

    fn draw_spans(
        &self,
        skia_surface: &mut skia_safe::Surface,
        prepared: Vec<PreparedSpan>,
        raster_scale: f32,
    ) -> Result<(), String> {
        for span in prepared {
            if let Some(instance) = &span.variable_instance {
                let typeface = instance.face.typeface.as_skia();
                surface::draw_span(skia_surface.canvas(), &span, typeface, raster_scale)?;
                continue;
            }
            let Some(face) = self.ensure_face(span.font)? else {
                return Err(format!(
                    "cached SVG font face {:?} became unavailable",
                    span.font
                ));
            };
            let typeface = face.typeface.as_skia();
            surface::draw_span(skia_surface.canvas(), &span, typeface, raster_scale)?;
        }
        Ok(())
    }
}

pub(super) fn wrap_pixmap<'a>(
    pixmap: &'a mut PixmapMut<'_>,
) -> Result<Borrows<'a, Surface>, String> {
    let info = ImageInfo::new(
        (pixmap.width() as i32, pixmap.height() as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    surfaces::wrap_pixels(&info, pixmap.data_mut(), info.min_row_bytes(), None)
        .ok_or_else(|| "Skia text surface allocation failed".to_string())
}

fn clip_crop(canvas: &Canvas, crop: &surface::Crop) {
    canvas.clip_rect(
        skia_safe::Rect::from_xywh(
            crop.x as f32,
            crop.y as f32,
            crop.width as f32,
            crop.height as f32,
        ),
        None,
        false,
    );
}

pub(super) fn uniform_raster_scale(transform: Transform) -> f32 {
    if transform.sx.is_finite()
        && transform.sx > 0.0
        && transform.sx == transform.sy
        && transform.kx == 0.0
        && transform.ky == 0.0
    {
        transform.sx
    } else {
        1.0
    }
}

fn skia_transforms(transform: Transform) -> (f32, Transform) {
    let raster_scale = uniform_raster_scale(transform);
    let canvas = if raster_scale == 1.0 {
        transform
    } else {
        transform.pre_scale(1.0 / raster_scale, 1.0 / raster_scale)
    };
    (raster_scale, canvas)
}
