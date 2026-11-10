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

fn positioned_glyphs(tree: &usvg::Tree) -> Result<&[usvg::layout::PositionedGlyph], String> {
    text(tree)?
        .layouted()
        .first()
        .map(|span| span.positioned_glyphs.as_slice())
        .ok_or_else(|| "span missing".to_owned())
}

#[test]
fn text_length_on_ligature_tail_preserves_the_whole_cluster() -> Result<(), String> {
    let reference = parse("", "f<tspan>i</tspan>")?;
    let adjusted = parse(
        "",
        "f<tspan textLength='80' lengthAdjust='spacingAndGlyphs'>i</tspan>",
    )?;
    let chunk = text(&adjusted)?.chunks().first().ok_or("chunk missing")?;
    assert_eq!(chunk.text(), "fi");
    assert!(
        chunk
            .spans()
            .iter()
            .any(|span| span.start() == 1 && span.end() == 2)
    );
    let expected = single_glyph(&reference)?;
    let actual = single_glyph(&adjusted)?;
    for scalar in ["f", "i"] {
        assert_ne!(expected.id, single_glyph(&parse("", scalar)?)?.id);
    }
    assert_eq!(actual.id, expected.id);
    assert_eq!(actual.transform(), expected.transform());
    Ok(())
}

fn single_glyph(tree: &usvg::Tree) -> Result<&usvg::layout::PositionedGlyph, String> {
    let [glyph] = positioned_glyphs(tree)? else {
        return Err("expected exactly one glyph".to_owned());
    };
    Ok(glyph)
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

#[test]
fn tiny_glyph_length_skips_scaling_but_larger_length_scales() -> Result<(), String> {
    let natural = parse("style='font-kerning:none'", "HH")?;
    let tiny = parse(
        "style='font-kerning:none' textLength='0.00001px' lengthAdjust='spacingAndGlyphs'",
        "HH",
    )?;
    let scaled = parse(
        "style='font-kerning:none' textLength='1px' lengthAdjust='spacingAndGlyphs'",
        "HH",
    )?;
    assert_tiny_text_length(&tiny)?;
    let (first, second) = pair(&natural)?;
    let (tiny_first, tiny_second) = pair(&tiny)?;
    let (scaled_first, _) = pair(&scaled)?;
    let (advance, natural_scale) = natural_metrics(first, second);
    let width = advance * 2.0;
    assert!(0.00001 / width < 0.001 && 1.0 / width >= 0.001);
    for (original, adjusted) in [(first, tiny_first), (second, tiny_second)] {
        assert_eq!(original.id, adjusted.id);
        near(original.transform().sx, adjusted.transform().sx);
        near(original.transform().tx, adjusted.transform().tx);
    }
    assert!(scaled_first.transform().sx < natural_scale);
    Ok(())
}

fn assert_tiny_text_length(tree: &usvg::Tree) -> Result<(), String> {
    let span = text(tree)?
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .ok_or("text span missing")?;
    assert_eq!(span.text_length(), Some(0.00001));
    assert_eq!(span.length_adjust(), usvg::LengthAdjust::SpacingAndGlyphs);
    Ok(())
}

#[test]
fn negative_spacing_removes_only_the_first_glyph() -> Result<(), String> {
    let natural = parse("style='font-kerning:none'", "Hi")?;
    let (first, second) = pair(&natural)?;
    assert_eq!((first.text.as_str(), second.text.as_str()), ("H", "i"));
    assert_ne!(first.id, second.id);
    let advance = second.transform().tx - first.transform().tx;
    assert!(advance > 0.0 && advance < 100.0);
    let spaced = parse("style='font-kerning:none' letter-spacing='-100'", "Hi")?;
    let glyphs = &text(&spaced)?
        .layouted()
        .first()
        .ok_or("span missing")?
        .positioned_glyphs;
    let [remaining] = glyphs.as_slice() else {
        return Err("expected one surviving glyph".to_owned());
    };
    assert_eq!(remaining.text, "i");
    assert_eq!(remaining.id, second.id);
    near(remaining.transform().tx, first.transform().tx);
    Ok(())
}

#[rstest::rstest]
#[case::regular_space("H H", 6, &[("H", 0.0), (" ", 0.0), ("H", 6.0)])]
#[case::negative_space("H H", -4, &[("H", 0.0), (" ", 0.0), ("H", -4.0)])]
#[case::multiple_spaces(
    "H H H",
    3,
    &[("H", 0.0), (" ", 0.0), ("H", 3.0), (" ", 3.0), ("H", 6.0)],
)]
#[case::no_space("HH", 9, &[("H", 0.0), ("H", 0.0)])]
#[case::nonbreaking_space(
    "H\u{00a0}H",
    4,
    &[("H", 0.0), ("\u{00a0}", 0.0), ("H", 4.0)],
)]
fn word_spacing_changes_only_advance_after_separators(
    #[case] content: &str,
    #[case] spacing: i32,
    #[case] expected: &[(&str, f32)],
) -> Result<(), String> {
    let common = "style='font-kerning:none' xml:space='preserve'";
    let natural = parse(common, content)?;
    let spaced = parse(&format!("word-spacing='{spacing}' {common}"), content)?;
    let natural_glyphs = positioned_glyphs(&natural)?;
    let spaced_glyphs = positioned_glyphs(&spaced)?;
    assert_eq!(natural_glyphs.len(), expected.len());
    assert_eq!(spaced_glyphs.len(), expected.len());
    for (index, &(glyph, displacement)) in expected.iter().enumerate() {
        let before = &natural_glyphs[index];
        let after = &spaced_glyphs[index];
        assert_eq!(before.text, glyph);
        assert_eq!(after.text, glyph);
        assert_eq!(before.id, after.id);
        near(before.transform().sx, after.transform().sx);
        near(after.transform().tx - before.transform().tx, displacement);
    }
    Ok(())
}

#[rstest::rstest]
#[case::shorter(1)]
#[case::longer(80)]
fn single_glyph_spacing_length_does_not_move_or_scale_glyph(
    #[case] length: u32,
) -> Result<(), String> {
    let natural = parse("style='font-kerning:none'", "H")?;
    let adjusted = parse(
        &format!("style='font-kerning:none' textLength='{length}' lengthAdjust='spacing'"),
        "H",
    )?;
    let span = text(&adjusted)?
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .ok_or("adjusted text span missing")?;
    assert_eq!(span.text_length(), Some(length as f32));
    assert_eq!(span.length_adjust(), usvg::LengthAdjust::Spacing);
    let [before] = positioned_glyphs(&natural)? else {
        return Err("expected one natural glyph".to_owned());
    };
    let [after] = positioned_glyphs(&adjusted)? else {
        return Err("expected one adjusted glyph".to_owned());
    };
    assert_eq!(before.id, after.id);
    near(before.transform().sx, after.transform().sx);
    near(before.transform().sy, after.transform().sy);
    near(before.transform().tx, after.transform().tx);
    near(before.transform().ty, after.transform().ty);
    Ok(())
}

#[test]
fn letter_spacing_is_ignored_for_devanagari_clusters() -> Result<(), String> {
    let natural = parse("style='font-kerning:none'", "\u{0915}\u{0915}")?;
    let tracked = parse(
        "letter-spacing='8' style='font-kerning:none'",
        "\u{0915}\u{0915}",
    )?;
    let natural_glyphs = positioned_glyphs(&natural)?;
    let tracked_glyphs = positioned_glyphs(&tracked)?;
    assert_eq!(natural_glyphs.len(), 2);
    assert_eq!(tracked_glyphs.len(), 2);
    for (before, after) in natural_glyphs.iter().zip(tracked_glyphs) {
        assert_eq!(before.id, after.id);
        near(before.transform().tx, after.transform().tx);
        near(before.transform().sx, after.transform().sx);
    }
    Ok(())
}
