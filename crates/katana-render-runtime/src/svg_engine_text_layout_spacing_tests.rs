use super::{near, parse, text, usvg};

const LETTER_SPACING: f32 = 4.0;
const TARGET_LENGTH: f32 = 80.0;

fn pair(
    tree: &usvg::Tree,
) -> Result<
    (
        &usvg::layout::PositionedGlyph,
        &usvg::layout::PositionedGlyph,
    ),
    String,
> {
    let span = text(tree)?.layouted().first().ok_or("span missing")?;
    match span.positioned_glyphs.as_slice() {
        [first, second] => Ok((first, second)),
        _ => Err("expected two glyphs".to_owned()),
    }
}

fn natural_metrics(
    first: &usvg::layout::PositionedGlyph,
    second: &usvg::layout::PositionedGlyph,
) -> (f32, f32) {
    assert_eq!(first.id, second.id);
    let scale = first.transform().sx;
    near(second.transform().sx, scale);
    let advance = second.transform().tx - first.transform().tx;
    assert!(advance > 0.0 && scale > 0.0);
    (advance, scale)
}

#[test]
fn letter_spacing_changes_distance_without_scaling_glyphs() -> Result<(), String> {
    let ordinary = parse("", "HH")?;
    let spaced = parse("letter-spacing='4'", "HH")?;
    let (first, second) = pair(&ordinary)?;
    let (spaced_first, spaced_second) = pair(&spaced)?;
    for (a, b) in [(first, spaced_first), (second, spaced_second)] {
        assert_eq!(a.id, b.id);
        near(a.transform().sx, b.transform().sx);
    }
    let gap = second.transform().tx - first.transform().tx;
    let spaced_gap = spaced_second.transform().tx - spaced_first.transform().tx;
    near(spaced_gap - gap, LETTER_SPACING);
    Ok(())
}

#[test]
fn text_length_spacing_and_glyph_scaling_have_distinct_effects() -> Result<(), String> {
    let ordinary = parse("style='font-kerning:none'", "HH")?;
    let spacing = parse(
        "style='font-kerning:none' textLength='80' lengthAdjust='spacing'",
        "HH",
    )?;
    let scaling = parse(
        "style='font-kerning:none' textLength='80' lengthAdjust='spacingAndGlyphs'",
        "HH",
    )?;
    for tree in [&ordinary, &spacing, &scaling] {
        assert_kerning_disabled(tree)?;
    }
    assert_target_lengths(&ordinary, &spacing, &scaling)
}

fn assert_target_lengths(
    ordinary: &usvg::Tree,
    spacing: &usvg::Tree,
    scaling: &usvg::Tree,
) -> Result<(), String> {
    let (first, second) = pair(ordinary)?;
    let (spaced_first, spaced_second) = pair(spacing)?;
    let (scaled_first, scaled_second) = pair(scaling)?;
    for glyph in [spaced_first, spaced_second, scaled_first, scaled_second] {
        assert_eq!(first.id, glyph.id);
    }
    let (advance, natural_scale) = natural_metrics(first, second);
    near(spaced_first.transform().sx, natural_scale);
    near(spaced_second.transform().sx, natural_scale);
    near(scaled_first.transform().sx, scaled_second.transform().sx);
    let spaced_gap = spaced_second.transform().tx - spaced_first.transform().tx;
    near(spaced_gap + advance, TARGET_LENGTH);
    let natural_length = advance + advance;
    near(
        natural_length * scaled_first.transform().sx / natural_scale,
        TARGET_LENGTH,
    );
    Ok(())
}

fn assert_kerning_disabled(tree: &usvg::Tree) -> Result<(), String> {
    let span = text(tree)?
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .ok_or("text span missing")?;
    assert!(!span.apply_kerning());
    Ok(())
}
