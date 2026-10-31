use super::writer_coverage_tests::{assert_attribute, assert_pixels_equal};
use super::{first_text, parse_writer_svg, usvg};

const MPLUS_X_HEIGHT_UNITS: f32 = 520.0;
const MPLUS_UNITS_PER_EM: f32 = 1000.0;
const MPLUS_FONT_SIZE: f32 = 64.0;
const MPLUS_EX_OFFSET: f32 = 3.1;
const EXPECTED_MPLUS_BASELINE_Y: f32 =
    MPLUS_EX_OFFSET * MPLUS_X_HEIGHT_UNITS / MPLUS_UNITS_PER_EM * MPLUS_FONT_SIZE;
const YELLOWTAIL_ASCENT_UNITS: f32 = 1990.0;
const YELLOWTAIL_DESCENT_UNITS: f32 = -617.0;
const YELLOWTAIL_UNITS_PER_EM: f32 = 2048.0;
const YELLOWTAIL_FONT_SIZE: f32 = 20.0;
const YELLOWTAIL_X_HEIGHT_FALLBACK_RATIO: f32 = 0.45;
const MIDDLE_BASELINE_FRACTION: f32 = 0.5;
const TEXT_LAYOUT_TOLERANCE: f32 = 0.0001;

#[test]
fn writer_text_ex_uses_selected_font_glyph_x_height() -> Result<(), String> {
    let input = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="160"><text x="12" y="{MPLUS_EX_OFFSET}ex" font-family="'Mplus 1p'" font-size="{MPLUS_FONT_SIZE}">x</text></svg>"#
    );
    let original = parse_writer_svg(&input, "writer-text-ex-mplus")?;
    assert_mplus_baseline(&original, "MPLUS baseline")?;
    let options = preserving_text_options();
    let serialized = original.to_string(&options);
    let reparsed = parse_writer_svg(&serialized, "writer-text-ex-mplus-roundtrip")?;
    assert_mplus_baseline(&reparsed, "reparsed MPLUS baseline")?;
    assert_pixels_equal(&original, &reparsed, "MPLUS ex baseline roundtrip")?;
    Ok(())
}

#[test]
fn writer_middle_baseline_uses_font_metric_x_height_fallback() -> Result<(), String> {
    let alphabetic = parse_writer_svg(&yellowtail_text_svg("alphabetic"), "yellowtail-alpha")?;
    let middle = parse_writer_svg(&yellowtail_text_svg("middle"), "yellowtail-middle")?;
    assert_text_baseline_delta(&alphabetic, &middle, "Yellowtail middle baseline")?;
    let serialized = middle.to_string(&preserving_text_options());
    assert_attribute(&serialized, "tspan", "dominant-baseline", "middle")?;
    let reparsed = parse_writer_svg(&serialized, "yellowtail-middle-roundtrip")?;
    assert_text_baseline_delta(
        &alphabetic,
        &reparsed,
        "reparsed Yellowtail middle baseline",
    )?;
    assert_pixels_equal(&middle, &reparsed, "Yellowtail middle baseline roundtrip")?;
    Ok(())
}

fn yellowtail_text_svg(dominant_baseline: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="64"><text x="12" y="32" font-family="Yellowtail" font-size="{YELLOWTAIL_FONT_SIZE}" dominant-baseline="{dominant_baseline}">x</text></svg>"#
    )
}

fn preserving_text_options() -> usvg::WriteOptions {
    usvg::WriteOptions {
        preserve_text: true,
        ..usvg::WriteOptions::default()
    }
}

fn first_text_chunk_y(tree: &usvg::Tree) -> Result<f32, String> {
    first_text(tree.root())
        .and_then(|text| text.chunks().first())
        .and_then(usvg::TextChunk::y)
        .ok_or_else(|| "writer text chunk y coordinate missing".to_string())
}

fn first_positioned_glyph_y(tree: &usvg::Tree) -> Result<f32, String> {
    first_text(tree.root())
        .and_then(|text| text.layouted().first())
        .and_then(|span| span.positioned_glyphs.first())
        .map(|glyph| glyph.transform().ty)
        .ok_or_else(|| "writer positioned glyph missing".to_string())
}

fn assert_text_baseline_delta(
    alphabetic: &usvg::Tree,
    middle: &usvg::Tree,
    case: &str,
) -> Result<(), String> {
    let delta = first_positioned_glyph_y(middle)? - first_positioned_glyph_y(alphabetic)?;
    assert_close(expected_yellowtail_middle_shift(), delta, case);
    Ok(())
}

fn assert_mplus_baseline(tree: &usvg::Tree, case: &str) -> Result<(), String> {
    assert_close(EXPECTED_MPLUS_BASELINE_Y, first_text_chunk_y(tree)?, case);
    assert_close(
        EXPECTED_MPLUS_BASELINE_Y,
        first_positioned_glyph_y(tree)?,
        case,
    );
    Ok(())
}

fn expected_yellowtail_middle_shift() -> f32 {
    let fallback_x_height = ((YELLOWTAIL_ASCENT_UNITS - YELLOWTAIL_DESCENT_UNITS)
        * YELLOWTAIL_X_HEIGHT_FALLBACK_RATIO)
        .round();
    fallback_x_height * YELLOWTAIL_FONT_SIZE / YELLOWTAIL_UNITS_PER_EM * MIDDLE_BASELINE_FRACTION
}

fn assert_close(expected: f32, actual: f32, case: &str) {
    assert!(
        (expected - actual).abs() <= TEXT_LAYOUT_TOLERANCE,
        "{case}: expected {expected}, got {actual}"
    );
}
