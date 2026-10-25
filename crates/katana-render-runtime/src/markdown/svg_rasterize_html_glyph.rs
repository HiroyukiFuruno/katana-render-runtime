use super::composite::TextRasterizer;
use resvg::usvg;
use skia_safe::{AlphaType, ColorType, Data, FontMgr, ImageInfo, surfaces};
use skrifa::{FontRef, MetadataProvider, raw::TableProvider};
use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tiny_skia::{PixmapMut, Transform};
use types::{CachedFace, CachedTypeface, FaceMetadata, SpanInput};

const RGBA_CHANNELS: usize = 4;
const RED_CHANNEL: usize = 0;
const GREEN_CHANNEL: usize = 1;
const BLUE_CHANNEL: usize = 2;
const ALPHA_CHANNEL: usize = 3;

thread_local! {
    /* WHY: 選択済みの物理データの復号だけに使い、frameごとのsystem font列挙を避ける。 */
    static HTML_FONT_MANAGER: FontMgr = FontMgr::default();
}

#[path = "svg_rasterize_html_glyph_anchor.rs"]
mod anchor;
#[path = "svg_rasterize_html_glyph_baseline.rs"]
mod baseline;
#[path = "svg_rasterize_html_glyph_paint.rs"]
mod paint;
#[path = "svg_rasterize_html_glyph_runs.rs"]
mod runs;
#[path = "svg_rasterize_html_glyph_surface.rs"]
mod surface;
use super::physical_font_face as ttc;
#[path = "svg_rasterize_html_glyph_types.rs"]
mod types;

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_ttc_integration_tests.rs"]
mod ttc_integration_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_glyph_typeface_tests.rs"]
mod typeface_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_glyph_coretext.rs"]
mod coretext;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_glyph_coretext_font.rs"]
mod coretext_font;

#[derive(Clone, Copy)]
enum RasterBackend {
    Skia,
    #[cfg(all(test, target_os = "macos"))]
    CoreText,
}

fn default_backend() -> RasterBackend {
    #[cfg(all(test, target_os = "macos"))]
    if std::env::var("KRR_HTML_FEATURE_CORETEXT_RASTER").as_deref() == Ok("1") {
        return RasterBackend::CoreText;
    }
    RasterBackend::Skia
}

pub(crate) struct HtmlGlyphRasterizer<'a> {
    database: &'a usvg::fontdb::Database,
    faces: RefCell<HashMap<usvg::fontdb::ID, Option<CachedFace>>>,
    font_mgr: OnceCell<FontMgr>,
    backend: RasterBackend,
    resvg_text_ids: HashSet<String>,
    authored_text_context: Option<&'a super::provenance::AuthoredTextContext>,
}

pub(super) fn has_nested_middle_candidate(tree: &usvg::Tree) -> bool {
    baseline::has_nested_middle_candidate(tree)
}

enum Prepared {
    Unsupported,
    Ready(Vec<PreparedSpan>),
}

impl<'a> HtmlGlyphRasterizer<'a> {
    pub(crate) fn new(database: &'a usvg::fontdb::Database) -> Self {
        Self {
            database,
            faces: RefCell::new(HashMap::new()),
            font_mgr: OnceCell::new(),
            backend: default_backend(),
            resvg_text_ids: HashSet::new(),
            authored_text_context: None,
        }
    }

    pub(crate) fn with_resvg_text(
        database: &'a usvg::fontdb::Database,
        resvg_text_ids: HashSet<String>,
    ) -> Self {
        Self {
            resvg_text_ids,
            ..Self::new(database)
        }
    }

    pub(super) fn with_resvg_text_and_source_context(
        database: &'a usvg::fontdb::Database,
        resvg_text_ids: HashSet<String>,
        authored_text_context: &'a super::provenance::AuthoredTextContext,
    ) -> Self {
        Self {
            resvg_text_ids,
            authored_text_context: Some(authored_text_context),
            ..Self::new(database)
        }
    }

    pub(super) fn has_authored_nested_text_context(&self, text: &usvg::Text) -> bool {
        self.authored_text_context
            .is_some_and(|context| context.matches(text))
    }

    #[cfg(all(test, target_os = "macos"))]
    pub(super) fn new_coretext(database: &'a usvg::fontdb::Database) -> Self {
        Self {
            database,
            faces: RefCell::new(HashMap::new()),
            font_mgr: OnceCell::new(),
            backend: RasterBackend::CoreText,
            resvg_text_ids: HashSet::new(),
            authored_text_context: None,
        }
    }

    fn prepare(&self, text: &usvg::Text) -> Result<Prepared, String> {
        let policy = baseline::resolved_policy(self, text)?;
        let mut prepared = Vec::new();
        for span in text
            .layouted()
            .iter()
            .filter(|span| !span.positioned_glyphs.is_empty())
        {
            if !self.prepare_span(span, policy, &mut prepared)? {
                return Ok(Prepared::Unsupported);
            }
        }
        self.apply_horizontal_alignment(text, &mut prepared);
        Ok(Prepared::Ready(prepared))
    }

    fn apply_horizontal_alignment(&self, text: &usvg::Text, prepared: &mut [PreparedSpan]) {
        if let Some(anchor_x) = single_horizontal_alignment(text, prepared) {
            let alignment = XAlignment {
                anchor_x,
                layout: anchor::layout_anchor(text),
                origin_snap_css_scale: anchor::run_origin_css_scale(text, prepared),
            };
            if let Some(span) = prepared.first_mut() {
                span.x_alignment = Some(alignment);
            }
        }
    }

    fn prepare_span(
        &self,
        span: &usvg::layout::Span,
        policy: Option<baseline::Policy>,
        prepared: &mut Vec<PreparedSpan>,
    ) -> Result<bool, String> {
        if !span_supported(span) {
            return Ok(false);
        }
        let Some((color, opacity)) = solid_fill(span) else {
            return Ok(false);
        };
        for run in runs::consecutive_runs(&span.positioned_glyphs) {
            let Some(input) = self.prepare_run_inputs(span, color, opacity, &run)? else {
                return Ok(false);
            };
            prepared.push(self.prepared_run(span, &run, input, policy));
        }
        Ok(true)
    }

    fn prepare_run_inputs(
        &self,
        span: &usvg::layout::Span,
        color: usvg::Color,
        opacity: u8,
        run: &runs::GlyphRun,
    ) -> Result<Option<SpanInput>, String> {
        let Some(face) = self.ensure_face(run.font)? else {
            return Ok(None);
        };
        let glyphs = &span.positioned_glyphs[run.start..run.end];
        if !run.font_size.is_finite() || run.font_size <= 0.0 {
            return Err("SVG text span has invalid font size".to_string());
        }
        let Some(glyph_ids) = glyph_ids(glyphs) else {
            return Ok(None);
        };
        Ok(Some(SpanInput {
            color,
            opacity,
            font: run.font,
            face,
            font_size: run.font_size,
            glyph_ids,
        }))
    }

    fn prepared_run(
        &self,
        span: &usvg::layout::Span,
        run: &runs::GlyphRun,
        input: SpanInput,
        policy: Option<baseline::Policy>,
    ) -> PreparedSpan {
        let baseline_shift = policy
            .map(|value| baseline::glyph_shift(self.database, input.font, &input.face, value))
            .unwrap_or(0.0);
        PreparedSpan {
            fill: [
                input.color.red,
                input.color.green,
                input.color.blue,
                input.opacity,
            ],
            font: input.font,
            glyphs: span.positioned_glyphs[run.start..run.end].to_vec(),
            glyph_ids: input.glyph_ids,
            upem: input.face.upem,
            font_size: input.font_size,
            baseline_shift,
            x_alignment: None,
        }
    }

    fn ensure_face(&self, id: usvg::fontdb::ID) -> Result<Option<CachedFace>, String> {
        if let Some(face) = self.faces.borrow().get(&id) {
            return Ok(face.clone());
        }
        let Some((path, index)) = self.face_file(id)? else {
            self.faces.borrow_mut().insert(id, None);
            return Ok(None);
        };
        let metadata = self.face_metadata(id)?;
        if metadata.unsupported {
            self.faces.borrow_mut().insert(id, None);
            return Ok(None);
        }
        let data = self.map_face_data(id, &path)?;
        let Some(typeface) = self.create_backend_face(&data, index)? else {
            self.faces.borrow_mut().insert(id, None);
            return Ok(None);
        };
        let face = CachedFace {
            typeface,
            upem: metadata.upem,
            ascent: metadata.ascent,
            descent: metadata.descent,
            x_height: metadata.x_height,
        };
        self.faces.borrow_mut().insert(id, Some(face.clone()));
        Ok(Some(face))
    }

    fn create_backend_face(
        &self,
        data: &Data,
        index: u32,
    ) -> Result<Option<CachedTypeface>, String> {
        let typeface = match self.backend {
            RasterBackend::Skia => {
                let font_mgr = self
                    .font_mgr
                    .get_or_init(|| HTML_FONT_MANAGER.with(Clone::clone));
                let Some(typeface) = ttc::create_physical_typeface(data, index, font_mgr)? else {
                    return Ok(None);
                };
                CachedTypeface::Skia(typeface)
            }
            #[cfg(all(test, target_os = "macos"))]
            RasterBackend::CoreText => CachedTypeface::CoreText(
                coretext_font::create_typeface(data.as_bytes(), index)?
                    .ok_or_else(|| "CoreText physical font creation failed".to_string())?,
            ),
        };
        Ok(Some(typeface))
    }

    fn face_file(&self, id: usvg::fontdb::ID) -> Result<Option<(std::path::PathBuf, u32)>, String> {
        let Some((source, index)) = self.database.face_source(id) else {
            return Err(format!("SVG font face {id:?} is unavailable"));
        };
        match source {
            usvg::fontdb::Source::File(path) | usvg::fontdb::Source::SharedFile(path, _) => {
                Ok(Some((path, index)))
            }
            usvg::fontdb::Source::Binary(_) => Ok(None),
        }
    }

    fn face_metadata(&self, id: usvg::fontdb::ID) -> Result<FaceMetadata, String> {
        let Some((upem, variable, color_or_bitmap, metrics)) = self
            .database
            .with_face_data(id, |bytes, face_index| {
                let font = FontRef::from_index(bytes, face_index).ok()?;
                Some((
                    font.head().ok()?.units_per_em() as f32,
                    !font.axes().is_empty(),
                    font.colr().is_ok() || font.svg().is_ok() || !font.bitmap_strikes().is_empty(),
                    font.metrics(
                        skrifa::prelude::Size::unscaled(),
                        &font.axes().location(std::iter::empty::<(&str, f32)>()),
                    ),
                ))
            })
            .flatten()
        else {
            return Err(format!("SVG font face {id:?} cannot be parsed"));
        };
        Ok(FaceMetadata {
            upem,
            unsupported: variable || color_or_bitmap,
            ascent: metrics.ascent.round(),
            descent: metrics.descent.round(),
            x_height: metrics.x_height,
        })
    }

    fn map_face_data(&self, id: usvg::fontdb::ID, path: &Path) -> Result<Data, String> {
        let Some(data) = Data::from_filename(path) else {
            return Err(format!("SVG font file {} cannot be mapped", path.display()));
        };
        let Some(same_bytes) = self
            .database
            .with_face_data(id, |bytes, _| bytes == data.as_bytes())
        else {
            return Err(format!("SVG font face {id:?} data is unavailable"));
        };
        if !same_bytes {
            return Err(format!(
                "SVG font file {} differs from parsed face",
                path.display()
            ));
        }
        Ok(data)
    }
}

fn glyph_ids(glyphs: &[usvg::layout::PositionedGlyph]) -> Option<Vec<u16>> {
    glyphs
        .iter()
        .map(|glyph| u16::try_from(glyph.id.0).ok())
        .collect()
}

fn span_supported(span: &usvg::layout::Span) -> bool {
    span.visible
        && span.stroke.is_none()
        && span.underline.is_none()
        && span.overline.is_none()
        && span.line_through.is_none()
        && span.paint_order == usvg::PaintOrder::FillAndStroke
}

fn solid_fill(span: &usvg::layout::Span) -> Option<(usvg::Color, u8)> {
    let fill = span.fill.as_ref()?;
    let usvg::Paint::Color(color) = fill.paint() else {
        return None;
    };
    Some((*color, fill.opacity().to_u8()))
}

#[derive(Clone, Copy)]
struct XAlignment {
    anchor_x: f32,
    layout: Option<anchor::LayoutAnchor>,
    origin_snap_css_scale: Option<f32>,
}

fn single_horizontal_alignment(text: &usvg::Text, prepared: &[PreparedSpan]) -> Option<f32> {
    let [span] = prepared else { return None };
    let [chunk] = text.chunks() else { return None };
    let [source_span] = chunk.spans() else {
        return None;
    };
    if !alignment_text_supported(text, chunk, source_span, span) {
        return None;
    }
    if !horizontal_glyphs(&span.glyphs) {
        return None;
    }
    let anchor_x = chunk.x().unwrap_or(0.0);
    anchor_x.is_finite().then_some(anchor_x)
}

fn alignment_text_supported(
    text: &usvg::Text,
    chunk: &usvg::TextChunk,
    source_span: &usvg::TextSpan,
    prepared: &PreparedSpan,
) -> bool {
    text.layouted().len() == 1
        && alignment_positioning_supported(text, chunk)
        && alignment_source_span_supported(source_span)
        && !prepared.glyphs.is_empty()
}

fn alignment_positioning_supported(text: &usvg::Text, chunk: &usvg::TextChunk) -> bool {
    all_zero(text.dx())
        && alignment_dy_supported(text.dy())
        && all_zero(text.rotate())
        && text.writing_mode() == usvg::WritingMode::LeftToRight
        && matches!(chunk.text_flow(), usvg::TextFlow::Linear)
}

fn alignment_source_span_supported(source_span: &usvg::TextSpan) -> bool {
    source_span.letter_spacing() == 0.0
        && source_span.word_spacing() == 0.0
        && source_span.text_length().is_none()
}

fn alignment_dy_supported(values: &[f32]) -> bool {
    values.first().is_none_or(|first| first.is_finite())
        && values.iter().skip(1).all(|value| *value == 0.0)
}

fn all_zero(values: &[f32]) -> bool {
    values.iter().all(|value| *value == 0.0)
}

fn horizontal_glyphs(glyphs: &[usvg::layout::PositionedGlyph]) -> bool {
    glyphs.iter().all(|glyph| {
        let transform = glyph.transform();
        transform.sx.is_finite()
            && transform.sy.is_finite()
            && transform.sx > 0.0
            && transform.sy > 0.0
            && transform.sx == transform.sy
            && transform.kx == 0.0
            && transform.ky == 0.0
    })
}

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
        if prepared.is_empty() {
            return Ok(true);
        }
        let full_transform = transform.pre_concat(text.flattened().transform());
        self.render_prepared(prepared, text, full_transform, pixmap)
    }
}

impl HtmlGlyphRasterizer<'_> {
    fn render_prepared(
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
        ) else {
            return Ok(true);
        };
        let pixels = self.draw_prepared(prepared, full_transform, &crop)?;
        surface::composite_rgba(pixmap, &pixels, crop)?;
        Ok(true)
    }

    fn draw_prepared(
        &self,
        prepared: Vec<PreparedSpan>,
        full_transform: Transform,
        crop: &surface::Crop,
    ) -> Result<Vec<u8>, String> {
        #[cfg(all(test, target_os = "macos"))]
        if matches!(self.backend, RasterBackend::CoreText) {
            return coretext::draw_prepared(self, prepared, full_transform, crop);
        }
        self.draw_skia_prepared(prepared, full_transform, crop)
    }

    fn draw_skia_prepared(
        &self,
        prepared: Vec<PreparedSpan>,
        full_transform: Transform,
        crop: &surface::Crop,
    ) -> Result<Vec<u8>, String> {
        /* WHY: BlinkのSVG文字と同様に、等方倍率をfont sizeへ移してstrike選択を揃える。 */
        let (raster_scale, canvas_transform) = skia_transforms(full_transform);
        let mut pixels = vec![0; crop.width as usize * crop.height as usize * RGBA_CHANNELS];
        let info = ImageInfo::new(
            (crop.width as i32, crop.height as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        let mut skia_surface =
            surfaces::wrap_pixels(&info, pixels.as_mut_slice(), info.min_row_bytes(), None)
                .ok_or_else(|| "Skia text surface allocation failed".to_string())?;
        let matrix = surface::skia_matrix(canvas_transform, crop.x as f32, crop.y as f32);
        let matrix = skia_safe::M44::from(&matrix);
        skia_surface.canvas().set_matrix(&matrix);
        self.draw_spans(&mut skia_surface, prepared, raster_scale)?;
        drop(skia_surface);
        Ok(pixels)
    }

    fn draw_spans(
        &self,
        skia_surface: &mut skia_safe::Surface,
        prepared: Vec<PreparedSpan>,
        raster_scale: f32,
    ) -> Result<(), String> {
        for span in prepared {
            let Some(face) = self.ensure_face(span.font)? else {
                return Err(format!(
                    "cached SVG font face {:?} became unavailable",
                    span.font
                ));
            };
            let typeface = face.typeface.as_skia()?;
            surface::draw_span(skia_surface.canvas(), &span, typeface, raster_scale)?;
        }
        Ok(())
    }
}

fn uniform_raster_scale(transform: Transform) -> f32 {
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

struct PreparedSpan {
    fill: [u8; RGBA_CHANNELS],
    font: usvg::fontdb::ID,
    glyphs: Vec<usvg::layout::PositionedGlyph>,
    glyph_ids: Vec<u16>,
    upem: f32,
    font_size: f32,
    baseline_shift: f32,
    x_alignment: Option<XAlignment>,
}
