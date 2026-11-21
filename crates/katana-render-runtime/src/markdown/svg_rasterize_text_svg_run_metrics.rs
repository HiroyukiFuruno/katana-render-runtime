use super::super::features::features_for_face;
use super::svg_font_instance::svg_font_instance;
use super::{FontShape, HARFRUST_SCALE, face_cache, shape_buffer_with_features};
use resvg::usvg;

#[path = "svg_rasterize_text_svg_fragment_run.rs"]
mod fragment_run;
pub(in crate::markdown::svg_rasterize) use fragment_run::shape_fragment_run;

const SVG_INK_BOUNDS_LENGTH: usize = 4;
const SVG_INK_BOUNDS_MAX_X_INDEX: usize = 2;
const SVG_INK_BOUNDS_MAX_Y_INDEX: usize = 3;
pub(crate) type SvgInkBounds = [f64; SVG_INK_BOUNDS_LENGTH];

#[derive(Clone, Copy)]
pub(crate) struct SvgTextRunMetrics {
    pub(crate) advance: f32,
    pub(crate) ascent: f32,
    pub(crate) descent: f32,
    pub(crate) layout_ascent: Option<f32>,
    pub(crate) ink_bounds: Option<SvgInkBounds>,
}

#[derive(Clone, Copy)]
pub(crate) struct SvgRunInkDiagnostic {
    pub(crate) bounds: Option<[f64; SVG_INK_BOUNDS_LENGTH]>,
    pub(crate) glyphs_without_extents: usize,
}

struct MeasuredSvgRun {
    metrics: SvgTextRunMetrics,
    ink: SvgRunInkDiagnostic,
    glyphs: harfrust::Buffer,
}

pub(in super::super) fn shape_svg_run_metrics_with_face(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    include_ink_diagnostic: bool,
) -> Option<(SvgTextRunMetrics, Option<SvgRunInkDiagnostic>)> {
    let (face, instance) = svg_font_instance(database, face_id, font).ok()?;
    let run = measure_svg_run(database, face_id, text, font, &instance).ok()?;
    /* WHY: metricsとshape取得後もsource世代の一致を確認してから結果を公開する。 */
    face_cache::commit_file_font(database, face_id, &face)
        .then_some((run.metrics, include_ink_diagnostic.then_some(run.ink)))
}

fn measure_svg_run(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    instance: &harfrust::font::Font,
) -> Result<MeasuredSvgRun, &'static str> {
    let (ascent, descent) =
        svg_run_line_metrics(instance, font.size).ok_or("SVG font line metrics unavailable")?;
    /* WHY: 同一instanceでshapeと輪郭計測を一度ずつ行う。 */
    let features = features_for_face(database, face_id, text, None, font.trim)
        .ok_or("SVG font features unavailable")?;
    let glyphs = shape_buffer_with_features(instance, text, font, &features)
        .ok_or("SVG font shaping failed")?;
    let mut metrics =
        svg_text_run_metrics(&glyphs, ascent, descent).ok_or("invalid SVG run metrics")?;
    let ink = svg_run_ink_diagnostic(instance, &glyphs, font.size);
    metrics.ink_bounds = ink.bounds;
    Ok(MeasuredSvgRun {
        metrics,
        ink,
        glyphs,
    })
}

fn svg_text_run_metrics(
    glyphs: &harfrust::Buffer,
    ascent: f32,
    descent: f32,
) -> Option<SvgTextRunMetrics> {
    let advance = glyphs
        .glyph_positions()
        .iter()
        .map(|position| position.x_advance as f32 / HARFRUST_SCALE)
        .sum::<f32>();
    if ![advance, ascent, descent].into_iter().all(f32::is_finite)
        || advance < 0.0
        || ascent + descent <= 0.0
    {
        return None;
    }
    Some(SvgTextRunMetrics {
        advance,
        ascent,
        descent,
        layout_ascent: None,
        ink_bounds: None,
    })
}

fn svg_run_line_metrics(instance: &harfrust::font::Font, size: f32) -> Option<(f32, f32)> {
    let line = instance.metrics().h_line()?;
    /* WHY: Harfrustのフォントモデルは無効なunitsPerEmを1000へ正規化する。 */
    let scale = size / f32::from(instance.units_per_em());
    let ascent = line.ascender.to_f32() * scale;
    let descent = -line.descender.to_f32() * scale;
    Some((ascent, descent))
}

pub(in super::super) fn svg_run_endpoints_with_face(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    font: FontShape,
) -> Option<(f32, f32)> {
    let (face, instance) = svg_font_instance(database, face_id, font).ok()?;
    let (ascent, descent) = svg_run_line_metrics(&instance, font.size)?;
    if ![ascent, descent].into_iter().all(f32::is_finite) || ascent + descent <= 0.0 {
        return None;
    }
    face_cache::commit_file_font(database, face_id, &face).then_some((ascent, descent))
}

fn svg_run_ink_diagnostic(
    instance: &harfrust::font::Font,
    glyphs: &harfrust::Buffer,
    size: f32,
) -> SvgRunInkDiagnostic {
    let shaper = harfrust::ShaperFont::new(instance).with_scale((size * HARFRUST_SCALE) as i32);
    let divisor = f64::from(HARFRUST_SCALE);
    let mut x = 0.0;
    let mut y = 0.0;
    let mut result = SvgRunInkDiagnostic {
        bounds: None,
        glyphs_without_extents: 0,
    };
    for (info, position) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
        if let Some(extents) = shaper.glyph_extents(harfrust::GlyphId::new(info.glyph_id)) {
            result.bounds = extend_ink_bounds(result.bounds, position, &extents, [x, y], divisor);
        } else {
            /* WHY: 空白等の輪郭なしを成功bboxへ隠さず診断に残す。 */
            result.glyphs_without_extents += 1;
        }
        x += f64::from(position.x_advance);
        y += f64::from(position.y_advance);
    }
    result
}

fn extend_ink_bounds(
    bounds: Option<[f64; SVG_INK_BOUNDS_LENGTH]>,
    position: &harfrust::GlyphPosition,
    extents: &harfrust::GlyphExtents,
    pen: [f64; 2],
    divisor: f64,
) -> Option<[f64; SVG_INK_BOUNDS_LENGTH]> {
    let left = pen[0] + f64::from(position.x_offset) + f64::from(extents.x_bearing);
    let top_up = pen[1] + f64::from(position.y_offset) + f64::from(extents.y_bearing);
    let right = left + f64::from(extents.width);
    let bottom_up = top_up + f64::from(extents.height);
    if extents.width == 0 || extents.height == 0 {
        return bounds;
    }
    let glyph_bounds = [
        left.min(right) / divisor,
        -top_up.max(bottom_up) / divisor,
        left.max(right) / divisor,
        -top_up.min(bottom_up) / divisor,
    ];
    Some(match bounds {
        Some(bounds) => [
            bounds[0].min(glyph_bounds[0]),
            bounds[1].min(glyph_bounds[1]),
            bounds[SVG_INK_BOUNDS_MAX_X_INDEX].max(glyph_bounds[SVG_INK_BOUNDS_MAX_X_INDEX]),
            bounds[SVG_INK_BOUNDS_MAX_Y_INDEX].max(glyph_bounds[SVG_INK_BOUNDS_MAX_Y_INDEX]),
        ],
        None => glyph_bounds,
    })
}

#[cfg(test)]
#[path = "svg_rasterize_text_svg_run_metrics_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_text_svg_run_metrics_ink_tests.rs"]
mod ink_diagnostic_contract_tests;
