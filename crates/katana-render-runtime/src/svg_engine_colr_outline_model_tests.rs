use super::super::{Builder, GlyphPainter};
use super::{FONT_BYTES, finish, font, painter, solid_brush};
use skrifa::{
    GlyphId, MetadataProvider,
    color::{Brush, ColorPainter, ColorStop, Extend, Transform},
    outline::OutlinePen,
    raw::types::Point,
};
use xmlwriter::XmlWriter;

const VALID_OUTLINE_GLYPH: u32 = 9;
const INVALID_GLYPH: u32 = u32::MAX;
const GLYPH_X_MIN: f32 = 100.0;
const GLYPH_Y_MIN: f32 = 250.0;
const GLYPH_X_MAX: f32 = 900.0;
const GLYPH_Y_MAX: f32 = 950.0;
const MOVE_X: f32 = 0.0;
const MOVE_Y: f32 = 0.0;
const QUAD_CONTROL_X: f32 = 1.0;
const QUAD_CONTROL_Y: f32 = 4.0;
const QUAD_END_X: f32 = 6.0;
const QUAD_END_Y: f32 = 2.0;
const CUBIC_CONTROL_X0: f32 = -2.0;
const CUBIC_CONTROL_Y0: f32 = 8.0;
const CUBIC_CONTROL_X1: f32 = 9.0;
const CUBIC_CONTROL_Y1: f32 = -3.0;
const CUBIC_END_X: f32 = 4.0;
const CUBIC_END_Y: f32 = 5.0;
const EXPECTED_CONTROL_POINT_BOUNDS: (f32, f32, f32, f32) = (-2.0, -3.0, 9.0, 8.0);
const EXPECTED_OUTLINE_PATH: &str = "M 0 0 Q 1 4 6 2 C -2 8 9 -3 4 5 Z";

fn assert_glyph_metadata(font: &skrifa::FontRef<'static>) -> Result<(), String> {
    if font
        .outline_glyphs()
        .get(GlyphId::new(VALID_OUTLINE_GLYPH))
        .is_none()
    {
        return Err("fixture metadata must contain the selected outline glyph".to_owned());
    }
    Ok(())
}

#[test]
fn singular_colr_outline_retains_finite_gradient_transform() -> Result<(), String> {
    let font = font()?;
    assert_glyph_metadata(&font)?;
    let mut svg = super::writer();
    let mut path = String::new();
    let mut painter = painter(&font, &mut svg, &mut path);
    painter.push_transform(Transform {
        xx: 0.0,
        dx: 5.0,
        ..Transform::default()
    });
    painter.fill_glyph(GlyphId::new(VALID_OUTLINE_GLYPH), None, linear_brush());
    painter.pop_transform();
    let output = finish(svg);
    assert!(output.contains("gradientTransform=\"matrix(0 0 0 1 5 0)\""));
    assert!(output.contains("fill=\"url(#lg1)\""));
    assert!(!output.contains("NaN"));
    Ok(())
}

fn linear_brush() -> Brush<'static> {
    Brush::LinearGradient {
        p0: Point { x: 0.0, y: 0.0 },
        p1: Point {
            x: GLYPH_X_MAX,
            y: 0.0,
        },
        color_stops: &[
            ColorStop {
                offset: 0.0,
                palette_index: u16::MAX,
                alpha: 1.0,
            },
            ColorStop {
                offset: 1.0,
                palette_index: u16::MAX,
                alpha: 0.0,
            },
        ],
        extend: Extend::Pad,
    }
}

fn assert_independent_outline_bounds(bounds: Option<tiny_skia_path::Rect>) -> Result<(), String> {
    let Some(bounds) = bounds else {
        return Err("fixture outline glyph must have finite clip bounds".to_owned());
    };
    if (bounds.left(), bounds.top(), bounds.right(), bounds.bottom())
        != (GLYPH_X_MIN, GLYPH_Y_MIN, GLYPH_X_MAX, GLYPH_Y_MAX)
    {
        return Err(format!(
            "COLR clip bounds differ from source glyf metadata: {bounds:?}"
        ));
    }
    Ok(())
}

fn assert_valid_glyph_clip(painter: &GlyphPainter<'_>) -> Result<(), String> {
    assert_independent_outline_bounds(painter.clip_stack.last().copied().flatten())?;
    if painter.path_buf.is_empty() {
        return Err("fixture outline glyph must produce a bounded clip path".to_owned());
    }
    Ok(())
}

fn assert_missing_glyph_clip(painter: &GlyphPainter<'_>) -> Result<(), String> {
    if painter.clip_stack.last() != Some(&None) || !painter.path_buf.is_empty() {
        return Err("missing glyph must produce an empty clip frame".to_owned());
    }
    Ok(())
}

fn assert_glyph_clip_sequence(
    font: &skrifa::FontRef<'static>,
    svg: &mut XmlWriter,
    path: &mut String,
) -> Result<(), String> {
    let mut painter = painter(font, svg, path);
    painter.push_clip_glyph(GlyphId::new(VALID_OUTLINE_GLYPH));
    assert_valid_glyph_clip(&painter)?;
    painter.pop_clip();
    painter.push_clip_glyph(GlyphId::new(INVALID_GLYPH));
    assert_missing_glyph_clip(&painter)?;
    painter.fill(solid_brush());
    painter.pop_clip();
    Ok(())
}

#[test]
fn real_font_outline_and_missing_glyph_have_distinct_clip_results() -> Result<(), String> {
    let font = font()?;
    assert_glyph_metadata(&font)?;
    let mut svg = super::writer();
    let mut path = String::new();
    assert_glyph_clip_sequence(&font, &mut svg, &mut path)?;
    let output = finish(svg);
    if output.contains("fill=\"rgb(17, 34, 51)\"") {
        return Err("missing glyph clip must suppress fill output".to_owned());
    }
    Ok(())
}

#[test]
fn malformed_glyf_outline_produces_an_empty_colr_clip() -> Result<(), String> {
    let bytes = font_with_truncated_outline_coordinates()?;
    let font = skrifa::FontRef::new(&bytes).map_err(|error| error.to_string())?;
    let glyph = GlyphId::new(VALID_OUTLINE_GLYPH);
    let mut svg = super::writer();
    let mut path = String::new();
    assert_malformed_outline_draw(&font, glyph, &mut path)?;
    let mut painter = painter(&font, &mut svg, &mut path);
    painter.push_clip_glyph(glyph);
    assert_missing_glyph_clip(&painter)?;
    painter.fill(solid_brush());
    painter.pop_clip();
    assert!(!finish(svg).contains("fill=\"rgb(17, 34, 51)\""));
    Ok(())
}

fn assert_malformed_outline_draw(
    font: &skrifa::FontRef<'_>,
    glyph: GlyphId,
    path: &mut String,
) -> Result<(), String> {
    use skrifa::{
        instance::{LocationRef, Size},
        outline::DrawSettings,
    };
    let outline = font
        .outline_glyphs()
        .get(glyph)
        .ok_or("outline metadata missing")?;
    assert!(
        outline
            .draw(
                DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                &mut Builder::new(path)
            )
            .is_err()
    );
    Ok(())
}

fn font_with_truncated_outline_coordinates() -> Result<Vec<u8>, String> {
    use rustybuzz::ttf_parser::{RawFace, Tag};
    use skrifa::raw::TableProvider;
    const FLAGS_OFFSET: usize = 14;
    const ORIGINAL_FLAGS: [u8; 4] = [0x37, 0x11, 0x21, 0x11];
    let raw = RawFace::parse(FONT_BYTES, 0).map_err(|error| format!("{error:?}"))?;
    let glyf = raw
        .table_records
        .into_iter()
        .find(|table| table.tag == Tag::from_bytes(b"glyf"))
        .ok_or("glyf table missing")?;
    let offset = font()?
        .loca(None)
        .map_err(|error| error.to_string())?
        .get_raw(VALID_OUTLINE_GLYPH as usize)
        .ok_or("glyph offset missing")?;
    let start = glyf.offset as usize + offset as usize + FLAGS_OFFSET;
    let mut bytes = FONT_BYTES.to_vec();
    let flags = &mut bytes[start..start + ORIGINAL_FLAGS.len()];
    assert_eq!(flags, &ORIGINAL_FLAGS);
    /* WHY: すべての点に長い座標を要求させ、実際の字形データより多くの座標を読もうとする。 */
    flags.fill(0);
    Ok(bytes)
}

#[test]
fn empty_outline_builder_has_no_bounds_and_finish_is_noop() -> Result<(), String> {
    let mut path = String::new();
    let bounds = {
        let mut builder = Builder::new(&mut path);
        let bounds = builder.bounds();
        builder.finish();
        bounds
    };
    if bounds.is_some() {
        return Err("empty outline builder must not have bounds".to_owned());
    }
    if !path.is_empty() {
        return Err("finishing an empty outline must leave its path empty".to_owned());
    }
    Ok(())
}

#[test]
fn outline_builder_keeps_control_points_in_path_and_conservative_bounds() -> Result<(), String> {
    let mut path = String::new();
    let bounds = control_point_outline(&mut path)
        .ok_or_else(|| "outline commands must produce bounds".to_owned())?;
    assert_control_point_bounds(bounds)?;
    if path != EXPECTED_OUTLINE_PATH {
        return Err(format!("outline command sequence mismatch: {path}"));
    }
    Ok(())
}

fn control_point_outline(path: &mut String) -> Option<tiny_skia_path::Rect> {
    let mut builder = Builder::new(path);
    builder.move_to(MOVE_X, MOVE_Y);
    builder.quad_to(QUAD_CONTROL_X, QUAD_CONTROL_Y, QUAD_END_X, QUAD_END_Y);
    builder.curve_to(
        CUBIC_CONTROL_X0,
        CUBIC_CONTROL_Y0,
        CUBIC_CONTROL_X1,
        CUBIC_CONTROL_Y1,
        CUBIC_END_X,
        CUBIC_END_Y,
    );
    builder.close();
    builder.finish();
    builder.bounds()
}

fn assert_control_point_bounds(bounds: tiny_skia_path::Rect) -> Result<(), String> {
    if (bounds.left(), bounds.top(), bounds.right(), bounds.bottom())
        != EXPECTED_CONTROL_POINT_BOUNDS
    {
        return Err(format!("control-point bounds mismatch: {bounds:?}"));
    }
    Ok(())
}
