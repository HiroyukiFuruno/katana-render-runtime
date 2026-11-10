use super::super::super::{Policy, PolicyKind};
use super::super::{HtmlGlyphRasterizer, shift};
use crate::markdown::svg_rasterize::font_file_fixture;
use resvg::usvg::Node;

const FONT_SIZE: f32 = 16.0;
#[cfg(target_os = "macos")]
const CJK_WITHOUT_X: &[u8] =
    include_bytes!("../../../../tests/fixtures/fonts/han-kerning/NotoSansCJKjp-han14.otf");
const VALID_CSS_SCALE: f32 = 1.0;
const INVALID_CSS_SCALES: &[Option<f32>] =
    &[None, Some(f32::NAN), Some(0.0), Some(-VALID_CSS_SCALE)];

#[test]
fn middle_shift_rejects_invalid_scales_with_a_valid_file_face() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = noto_file_fixture(&directory)?;
    let rasterizer = HtmlGlyphRasterizer::new(&fixture.database);
    let metadata = rasterizer.face_metadata(fixture.face_id)?;
    let calculate = |scale| {
        shift(
            &rasterizer,
            fixture.face_id,
            &metadata,
            middle_policy(scale),
            None,
        )
    };
    for &scale in INVALID_CSS_SCALES {
        assert_eq!(calculate(scale)?, None);
    }
    assert!(calculate(Some(VALID_CSS_SCALE))?.is_some_and(f32::is_finite));
    Ok(())
}

fn middle_policy(css_scale: Option<f32>) -> Policy {
    Policy {
        kind: PolicyKind::Middle,
        font_size: FONT_SIZE,
        css_scale,
        primary_shift: None,
    }
}

#[test]
fn tiny_svg_scale_rejects_middle_shift_when_css_font_size_rounds_to_zero() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = noto_file_fixture(&directory)?;
    for (scale, expected) in [(0.0005_f32, false), (0.001_f32, true)] {
        let actual = middle_shift_for_svg_scale(&fixture, scale)?;
        assert_eq!(
            actual.is_some_and(f32::is_finite),
            expected,
            "scale {scale}"
        );
    }
    Ok(())
}

fn noto_file_fixture(
    directory: &tempfile::TempDir,
) -> Result<font_file_fixture::FontFileFixture, String> {
    font_file_fixture::create(
        directory.path().join("NotoSans-Regular.ttf"),
        font_file_fixture::FONT_BYTES,
    )
    .map_err(|error| error.to_string())
}

fn middle_shift_for_svg_scale(
    fixture: &font_file_fixture::FontFileFixture,
    scale: f32,
) -> Result<Option<f32>, String> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" x="0" y="20" dy="1" font-family="Noto Sans" font-size="{FONT_SIZE}" dominant-baseline="middle" transform="scale({scale})">AB</text></svg>"#
    );
    let tree = fixture.parse_svg(&svg).map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("small-scale text missing".into());
    };
    let policy = super::super::super::uniform_policy(text).ok_or("middle policy missing")?;
    assert_eq!(policy.css_scale, Some(scale));
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let metadata = rasterizer.face_metadata(fixture.face_id)?;
    shift(&rasterizer, fixture.face_id, &metadata, policy, None)
}

#[cfg(target_os = "macos")]
#[test]
fn middle_without_x_glyph_preserves_original_glyph_origin() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = font_file_fixture::create(directory.path().join("han14.otf"), CJK_WITHOUT_X)
        .map_err(|error| error.to_string())?;
    let tree = cjk_middle_tree(&fixture)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("CJK middle text missing".into());
    };
    assert!(super::super::super::uniform_policy(text).is_some());
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    assert_middle_font_without_x(&rasterizer, fixture.face_id)?;
    assert!(super::super::super::resolved_policy(&rasterizer, text)?.is_none());
    let super::super::super::super::Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("CJK middle text not prepared".into());
    };
    let span = spans.first().ok_or("CJK glyph span missing")?;
    let glyph = span.glyphs.first().ok_or("CJK glyph missing")?;
    assert_eq!(glyph.font, fixture.face_id);
    assert_eq!(span.baseline_shift, 0.0);
    assert_eq!(
        super::super::super::super::paint::normalized_transform(span, glyph, 1.0).ty,
        glyph.transform().ty
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn cjk_middle_tree(
    fixture: &font_file_fixture::FontFileFixture,
) -> Result<resvg::usvg::Tree, String> {
    let family = &fixture
        .database
        .face(fixture.face_id)
        .and_then(|face| face.families.first())
        .ok_or("CJK family missing")?
        .0;
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" x="0" y="20" dy="1" font-family="{family}" font-size="{FONT_SIZE}" dominant-baseline="middle">あ</text></svg>"#
    );
    fixture
        .parse_svg(&source)
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn assert_middle_font_without_x(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: resvg::usvg::fontdb::ID,
) -> Result<(), String> {
    let metadata = rasterizer.face_metadata(id)?;
    assert!(!metadata.unsupported && super::super::raw_x_height(&metadata).is_some());
    let face = rasterizer.ensure_face(id)?.ok_or("CJK face missing")?;
    let typeface = face.typeface.as_skia()?;
    let font = super::super::paint::font_for_face(typeface, FONT_SIZE);
    assert_ne!(font.unichar_to_glyph('あ' as i32), 0);
    assert_eq!(font.unichar_to_glyph('x' as i32), 0);
    assert!(super::super::glyph_x_height(&font).is_none());
    Ok(())
}
