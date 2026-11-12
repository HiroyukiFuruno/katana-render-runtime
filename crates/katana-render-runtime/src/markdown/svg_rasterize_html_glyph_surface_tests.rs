use super::super::super::composite::TextRasterizer;
use super::{skia_matrix, transformed_text_bounds};
use resvg::usvg::{Node, Options, Tree};
use skia_safe::Point as SkiaPoint;
use tiny_skia::{Color, Pixmap, Transform};

const OFFSCREEN_WIDTH: u32 = 80;
const OFFSCREEN_HEIGHT: u32 = 40;
const OFFSCREEN_TRANSLATION: f32 = 4096.0;

#[test]
fn skia_matrix_maps_axes_and_crop_origin() {
    let matrix = skia_matrix(
        Transform::from_row(2.0, 3.0, 4.0, 5.0, 10.0, 20.0),
        7.0,
        11.0,
    );
    let mut values = [0.0; 9];
    matrix.get_9(&mut values);
    assert_eq!(values, [2.0, 4.0, 3.0, 3.0, 5.0, 9.0, 0.0, 0.0, 1.0]);
    let point = tiny_skia::Point::from_xy(1.25, -0.75);
    let mut tiny_point = point;
    let transform = Transform::from_row(2.0, 3.0, 4.0, 5.0, 10.0, 20.0);
    transform.map_point(&mut tiny_point);
    let skia_point = matrix.map_point(SkiaPoint::new(point.x, point.y));
    assert!((skia_point.x - (tiny_point.x - 7.0)).abs() < f32::EPSILON);
    assert!((skia_point.y - (tiny_point.y - 11.0)).abs() < f32::EPSILON);
}

#[test]
fn rotated_crop_union_has_exact_shifted_endpoints() -> Result<(), String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><text id="run" x="8" y="20" font-family="Helvetica" font-size="16">A</text></svg>"#;
    let options = Options {
        fontdb: crate::markdown::svg_rasterize::font::html_font_db_for_text("Helvetica", "A"),
        ..Options::default()
    };
    let tree = Tree::from_str(source, &options).map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("text node missing".to_owned());
    };
    let transform = Transform::from_row(0.96, 0.29, -0.29, 0.96, 4.0, 8.0);
    let base = transformed_text_bounds(text, transform, (0.0, 0.0), false).ok_or("empty bounds")?;
    let shifted = transformed_text_bounds(text, transform, (-1.0, 1.0), false)
        .ok_or("empty shifted bounds")?;
    let expected_dx = 0.29;
    let expected_dy = 0.96;
    assert!((shifted.0 - (base.0 - expected_dx)).abs() < 0.0001);
    assert!((shifted.1 - (base.1 - expected_dy)).abs() < 0.0001);
    assert!((shifted.2 - (base.2 + expected_dx)).abs() < 0.0001);
    assert!((shifted.3 - (base.3 + expected_dy)).abs() < 0.0001);
    Ok(())
}

#[test]
fn fully_offscreen_glyph_is_handled_as_noop_with_visible_control() -> Result<(), String> {
    let font_path = std::env::temp_dir().join(format!("krr-offscreen-{}.ttf", std::process::id()));
    let fixture = super::super::super::font_file_fixture::create(
        font_path,
        super::super::super::font_file_fixture::FONT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{OFFSCREEN_WIDTH}' height='{OFFSCREEN_HEIGHT}'><text id='run' x='8' y='24' font-family='Noto Sans' font-size='18'>Ag</text></svg>"
    );
    let tree = fixture.parse_svg(&svg).map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("offscreen text missing".to_owned());
    };
    let rasterizer = super::super::HtmlGlyphRasterizer::new(tree.fontdb());
    assert_visible_control(&rasterizer, text)?;
    assert_offscreen_noop(&rasterizer, text)
}

fn assert_visible_control(
    rasterizer: &super::super::HtmlGlyphRasterizer<'_>,
    text: &resvg::usvg::Text,
) -> Result<(), String> {
    let mut visible =
        Pixmap::new(OFFSCREEN_WIDTH, OFFSCREEN_HEIGHT).ok_or("visible pixmap unavailable")?;
    assert!(rasterizer.render(text, Transform::identity(), &mut visible.as_mut())?);
    assert!(
        visible
            .data()
            .chunks_exact(super::super::RGBA_CHANNELS)
            .any(|pixel| pixel[super::super::ALPHA_CHANNEL] != 0)
    );
    Ok(())
}

fn assert_offscreen_noop(
    rasterizer: &super::super::HtmlGlyphRasterizer<'_>,
    text: &resvg::usvg::Text,
) -> Result<(), String> {
    let mut offscreen =
        Pixmap::new(OFFSCREEN_WIDTH, OFFSCREEN_HEIGHT).ok_or("offscreen pixmap unavailable")?;
    offscreen.fill(Color::WHITE);
    let before = offscreen.data().to_vec();
    let translated = Transform::from_translate(OFFSCREEN_TRANSLATION, 0.0);
    assert!(rasterizer.render(text, translated, &mut offscreen.as_mut())?);
    assert_eq!(offscreen.data(), before);
    Ok(())
}

#[test]
fn text_crop_rejects_zero_canvas_and_unrepresentable_transformed_bounds() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = super::super::super::font_file_fixture::create(
        directory.path().join("NotoSans-Regular.ttf"),
        super::super::super::font_file_fixture::FONT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><text id="run" x="0" y="20" font-family="Noto Sans" font-size="16">AB</text></svg>"#;
    let tree = fixture.parse_svg(svg).map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("crop fixture text missing".into());
    };
    let crop = |transform, width, height| {
        super::text_crop(text, transform, width, height, (0.0, 0.0), false)
    };
    assert!(crop(Transform::identity(), OFFSCREEN_WIDTH, OFFSCREEN_HEIGHT).is_some());
    for (width, height) in [(0, OFFSCREEN_HEIGHT), (OFFSCREEN_WIDTH, 0)] {
        assert!(crop(Transform::identity(), width, height).is_none());
    }
    let huge = Transform::from_row(1.0e9, 0.0, 0.0, 1.0, -8.0e9, 0.0);
    let bounds = transformed_text_bounds(text, huge, (0.0, 0.0), false)
        .ok_or("transformed text bounds missing")?;
    assert!(bounds.0 < i32::MIN as f32 && bounds.2 > i32::MAX as f32);
    assert!(crop(huge, OFFSCREEN_WIDTH, OFFSCREEN_HEIGHT).is_none());
    Ok(())
}

#[test]
fn vertical_glyph_outline_has_crop_only_when_stroked() -> Result<(), String> {
    use crate::markdown::svg_rasterize::font_file_fixture;

    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let font = font_file_fixture::create(
        directory.path().join("VerticalOutline.ttf"),
        &font_with_vertical_outline()?,
    )
    .map_err(|error| error.to_string())?;
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><text id="run" x="8" y="20" font-family="Noto Sans" font-size="16" fill="none" stroke="black" stroke-width="2">A</text></svg>"#;
    let tree = font.parse_svg(svg).map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("stroked glyph is missing".to_owned());
    };
    let crop = |include_stroke| {
        super::text_crop(
            text,
            Transform::identity(),
            OFFSCREEN_WIDTH,
            OFFSCREEN_HEIGHT,
            (0.0, 0.0),
            include_stroke,
        )
    };
    assert!(text.flattened().bounding_box().is_empty());
    assert!(crop(false).is_none());
    assert!(crop(true).is_some());
    Ok(())
}

fn font_with_vertical_outline() -> Result<Vec<u8>, String> {
    use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
    use write_fonts::{
        FontBuilder,
        read::{FontRef, TableProvider},
        types::Tag,
    };

    const GLYF: Tag = Tag::new(b"glyf");
    /* WHY: 点 (100, 0) と (100, 600) を閉じ、幅ゼロのTrueType輪郭を作る。 */
    const VERTICAL_OUTLINE: &[u8] = &[
        0, 1, 0, 100, 0, 0, 0, 100, 2, 88, 0, 1, 0, 0, 0x33, 0x11, 100, 2, 88,
    ];
    let original = FontRef::new(FONT_BYTES).map_err(|error| error.to_string())?;
    let range = outline_range(&original, 'A')?;
    let mut outlines = original
        .data_for_tag(GLYF)
        .ok_or("glyf missing")?
        .as_bytes()
        .to_vec();
    let slot = outlines.get_mut(range).ok_or("glyph slot missing")?;
    assert!(VERTICAL_OUTLINE.len() <= slot.len());
    slot.fill(0);
    slot[..VERTICAL_OUTLINE.len()].copy_from_slice(VERTICAL_OUTLINE);
    let mut builder = FontBuilder::new();
    builder.add_raw(GLYF, &outlines);
    Ok(builder.copy_missing_tables(original).build())
}

fn outline_range(
    font: &write_fonts::read::FontRef<'_>,
    character: char,
) -> Result<std::ops::Range<usize>, String> {
    use write_fonts::read::TableProvider;

    let index = font
        .cmap()
        .map_err(|error| error.to_string())?
        .map_codepoint(character)
        .ok_or("character mapping missing")?
        .to_u32() as usize;
    let loca = font.loca(None).map_err(|error| error.to_string())?;
    let start = loca.get_raw(index).ok_or("glyph start missing")? as usize;
    let end = loca.get_raw(index + 1).ok_or("glyph end missing")? as usize;
    Ok(start..end)
}
