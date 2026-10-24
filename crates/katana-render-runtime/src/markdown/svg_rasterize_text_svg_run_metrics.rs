use super::super::features::features_for_face;
use super::{FontShape, HARFRUST_SCALE, face_cache, font_variations, shape_buffer_with_features};
use resvg::usvg;

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

pub(in super::super) fn shape_svg_run_metrics_with_face(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    include_ink_diagnostic: bool,
) -> Option<(SvgTextRunMetrics, Option<SvgRunInkDiagnostic>)> {
    validate_shaping_size(font.size)?;
    let face = face_cache::font_for_face(database, face_id)?;
    let instance = face
        .instance_builder()
        .variations(font_variations(font))
        .build();
    let (ascent, descent) = svg_run_line_metrics(&instance, font.size)?;
    /* WHY: default属性のSVG runのみ対象とし、同一instanceのshapeを一度だけ行う。 */
    let features = features_for_face(database, face_id, text, None, font.trim)?;
    let glyphs = shape_buffer_with_features(&instance, text, font, &features)?;
    let metrics = svg_text_run_metrics(&glyphs, ascent, descent)?;
    let ink = svg_run_ink_diagnostic(&instance, &glyphs, font.size);
    let metrics = SvgTextRunMetrics {
        layout_ascent: None,
        ink_bounds: ink.bounds,
        ..metrics
    };
    /* WHY: metricsとshape取得後もsource世代の一致を確認してから結果を公開する。 */
    face_cache::commit_file_font(database, face_id, &face)
        .then_some((metrics, include_ink_diagnostic.then_some(ink)))
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

pub(super) fn validate_shaping_size(size: f32) -> Option<()> {
    let shaping_scale = size * HARFRUST_SCALE;
    if !size.is_finite()
        || size <= 0.0
        || !shaping_scale.is_finite()
        || f64::from(shaping_scale) > f64::from(i32::MAX)
        || shaping_scale as i32 == 0
    {
        return None;
    }
    Some(())
}

fn svg_run_line_metrics(instance: &harfrust::font::Font, size: f32) -> Option<(f32, f32)> {
    let line = instance.metrics().h_line()?;
    let upem = instance.units_per_em();
    if upem == 0 {
        return None;
    }
    let scale = size / f32::from(upem);
    let ascent = line.ascender.to_f32() * scale;
    let descent = -line.descender.to_f32() * scale;
    Some((ascent, descent))
}

pub(in super::super) fn svg_run_endpoints_with_face(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    font: FontShape,
) -> Option<(f32, f32)> {
    validate_shaping_size(font.size)?;
    let face = face_cache::font_for_face(database, face_id)?;
    let instance = face
        .instance_builder()
        .variations(font_variations(font))
        .build();
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
mod tests {
    use super::{FontShape, shape_svg_run_metrics_with_face, svg_run_endpoints_with_face};
    use resvg::usvg::fontdb::{Database, Source};
    use std::sync::Arc;

    const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

    #[test]
    fn real_font_sources_preserve_width_bits_and_raw_metric_endpoints() -> Result<(), String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/fonts/NotoSans-Regular.ttf");
        let oracle = rustybuzz::ttf_parser::Face::parse(FONT_BYTES, 0)
            .map_err(|error| format!("real font oracle failed: {error:?}"))?;
        assert_real_source_metrics(path, &oracle)?;
        Ok(())
    }

    fn assert_real_source_metrics(
        path: std::path::PathBuf,
        oracle: &rustybuzz::ttf_parser::Face<'_>,
    ) -> Result<(), String> {
        for source in real_font_sources(path) {
            assert_metrics_for_source(source, oracle)?;
        }
        Ok(())
    }

    fn real_font_sources(path: std::path::PathBuf) -> [Source; 3] {
        [
            Source::Binary(Arc::new(FONT_BYTES.to_vec())),
            Source::SharedFile(path.clone(), Arc::new(FONT_BYTES.to_vec())),
            Source::File(path),
        ]
    }

    fn assert_metrics_for_source(
        source: Source,
        oracle: &rustybuzz::ttf_parser::Face<'_>,
    ) -> Result<(), String> {
        let database = database_with_source(source)?;
        let id = database
            .faces()
            .next()
            .ok_or("installed test face missing")?
            .id;
        for (text, size, weight) in [("AV", 16.0, 400), ("ffi", 18.25, 700), ("DB", 14.0, 400)] {
            assert_one_run_matches_oracle(&database, id, oracle, text, size, weight)?;
        }
        Ok(())
    }

    fn database_with_source(source: Source) -> Result<Database, String> {
        let mut parsed = Database::new();
        parsed.load_font_data(FONT_BYTES.to_vec());
        let mut face = parsed
            .faces()
            .next()
            .ok_or("real font face missing")?
            .clone();
        face.source = source;
        let mut database = Database::new();
        database.push_face_info(face);
        Ok(database)
    }

    fn assert_one_run_matches_oracle(
        database: &Database,
        id: resvg::usvg::fontdb::ID,
        oracle: &rustybuzz::ttf_parser::Face<'_>,
        text: &str,
        size: f32,
        weight: u16,
    ) -> Result<(), String> {
        let font = FontShape::new(size, weight, false);
        let actual = shape_svg_run_metrics_with_face(database, id, text, font, false)
            .ok_or("raw SVG run failed")?
            .0;
        let width = super::super::shape_text_with_face(database, id, text, font, None)
            .ok_or("existing width failed")?;
        assert_eq!(actual.advance.to_bits(), width.to_bits());
        assert_raw_endpoints_match_oracle(actual, oracle, font.size);
        Ok(())
    }

    fn assert_raw_endpoints_match_oracle(
        actual: super::SvgTextRunMetrics,
        oracle: &rustybuzz::ttf_parser::Face<'_>,
        size: f32,
    ) {
        let scale = size / f32::from(oracle.units_per_em());
        assert!((actual.ascent - f32::from(oracle.ascender()) * scale).abs() < 0.0001);
        assert!((actual.descent + f32::from(oracle.descender()) * scale).abs() < 0.0001);
    }

    #[test]
    fn subunit_integer_shape_scale_is_rejected_for_a_usable_real_font() -> Result<(), String> {
        let mut database = Database::new();
        database.load_font_data(FONT_BYTES.to_vec());
        let face = database.faces().next().ok_or("real font face missing")?.id;
        assert!(
            shape_svg_run_metrics_with_face(
                &database,
                face,
                "DB",
                FontShape::new(16.0, 400, false),
                false
            )
            .is_some()
        );
        let tiny = FontShape::new(1.0e-9, 400, false);
        assert!(shape_svg_run_metrics_with_face(&database, face, "DB", tiny, false).is_none());
        assert!(svg_run_endpoints_with_face(&database, face, tiny).is_none());
        Ok(())
    }
}

#[cfg(test)]
#[path = "svg_rasterize_text_svg_run_metrics_ink_tests.rs"]
mod ink_diagnostic_contract_tests;
