use super::super::{FONT, TestResult, font_fixture_tests::fixture_database, requested_font, usvg};
use rustybuzz::ttf_parser::{Face, Tag};
use std::sync::Arc;

const VARIABLE_FONT: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/RobotoFlex.subset.ttf"
);
const EMOJI_FONT: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/NotoColorEmojiCBDT.subset.ttf"
);
const AXIS: [u8; 4] = *b"YTLC";
const MINIMUM: f32 = 416.0;
const DEFAULT: f32 = 514.0;
const MAXIMUM: f32 = 570.0;

fn selected_face_resolver(id: Option<usvg::fontdb::ID>) -> usvg::FontResolver<'static> {
    usvg::FontResolver {
        select_font: Box::new(move |_, _| id),
        ..Default::default()
    }
}

fn oracle(value: f32) -> TestResult<(f32, f32)> {
    let mut face = Face::parse(VARIABLE_FONT, 0)?;
    let tag = Tag::from_bytes(&AXIS);
    assert!(
        face.variation_axes()
            .into_iter()
            .any(|axis| axis.tag == tag)
    );
    face.set_variation(tag, value)
        .ok_or("variable axis missing")?;
    let upm = f32::from(face.units_per_em());
    let height = face.x_height().ok_or("oracle x-height missing")?;
    /* WHY: MVARの整数丸めと浮動小数点計算の差として、1フォント単位を許容する。 */
    Ok((f32::from(height) / upm, 1.0 / upm))
}

fn assert_variable_height(value: f32, expected: (f32, f32)) -> TestResult<f32> {
    let (mut database, id) = fixture_database(VARIABLE_FONT)?;
    let mut font = requested_font()?;
    font.variations = vec![usvg::FontVariation::new(AXIS, value)];
    let resolver = selected_face_resolver(Some(id));
    let actual = usvg::layout::css_font_x_height(&font, &resolver, &mut database)
        .ok_or("variable x-height unavailable")?;
    assert!(
        (actual - expected.0).abs() <= expected.1,
        "axis {value}: {actual} != {}",
        expected.0
    );
    Ok(actual)
}

#[test]
fn variable_axis_changes_css_x_height_against_independent_oracle() -> TestResult<()> {
    let minimum = oracle(MINIMUM)?;
    let default = oracle(DEFAULT)?;
    let maximum = oracle(MAXIMUM)?;
    assert_ne!(minimum.0, maximum.0, "fixture axis has no x-height effect");
    for (value, expected) in [(MINIMUM, minimum), (DEFAULT, default), (MAXIMUM, maximum)] {
        assert_variable_height(value, expected)?;
    }
    let (mut database, id) = fixture_database(VARIABLE_FONT)?;
    let mut font = requested_font()?;
    font.variations.clear();
    let actual =
        usvg::layout::css_font_x_height(&font, &selected_face_resolver(Some(id)), &mut database)
            .ok_or("default x-height unavailable")?;
    assert!((actual - default.0).abs() <= default.1);
    Ok(())
}

#[test]
fn svg_ex_uses_half_em_when_selected_font_has_no_x_glyph_or_x_height() -> TestResult<()> {
    let face = Face::parse(EMOJI_FONT, 0)?;
    assert_eq!(face.x_height(), Some(0));
    assert!(face.glyph_index('x').is_none());
    let (database, id) = fixture_database(EMOJI_FONT)?;
    let options = usvg::Options {
        fontdb: database,
        font_resolver: selected_face_resolver(Some(id)),
        ..usvg::Options::default()
    };
    let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='40'><rect id='sample' font-size='20' x='2ex' width='10' height='10'/></svg>";
    let tree = usvg::Tree::from_str(svg, &options)?;
    let sample = tree.node_by_id("sample").ok_or("sample path missing")?;
    assert_eq!(sample.abs_bounding_box().x(), 20.0);
    Ok(())
}

#[test]
fn missing_and_stale_resolver_faces_are_rejected() -> TestResult<()> {
    let (mut database, id) = fixture_database(FONT)?;
    let font = requested_font()?;
    let resolver = selected_face_resolver(Some(id));
    let height = usvg::layout::css_font_x_height(&font, &resolver, &mut database)
        .ok_or("Noto x-height unavailable")?;
    assert!(height.is_finite() && height > 0.0);
    assert!(
        usvg::layout::css_font_x_height(&font, &selected_face_resolver(None), &mut database)
            .is_none()
    );
    Arc::get_mut(&mut database)
        .ok_or("font database unexpectedly shared")?
        .remove_face(id);
    assert!(usvg::layout::css_font_x_height(&font, &resolver, &mut database).is_none());
    Ok(())
}
