use super::path_support::path_text_with_content;
use super::{TestResult, near, path_support::path_text, text, usvg};

#[test]
fn path_text_rejects_unescaped_xml_text() -> TestResult<()> {
    let valid = path_text_with_content(0, "A &amp; B")?;
    assert!(path_glyphs(text(&valid)?).any(|(value, _, _)| value == "A"));
    for malformed in ["A & B", "A < B"] {
        assert!(
            path_text_with_content(0, malformed).is_err(),
            "invalid XML accepted: {malformed}"
        );
    }
    Ok(())
}

#[test]
fn rotation_is_applied_to_a_glyph_following_a_horizontal_path() -> TestResult<()> {
    let base = path_text(0)?;
    let rotated = path_text(90)?;
    let mut glyphs = Vec::new();
    for tree in [&base, &rotated] {
        let node = text(tree)?;
        let chunk = node.chunks().first().ok_or("chunk missing")?;
        assert!(matches!(chunk.text_flow(), usvg::TextFlow::Path(_)));
        let span = node.layouted().first().ok_or("layout span missing")?;
        let [glyph] = span.positioned_glyphs.as_slice() else {
            return Err("expected one path glyph".into());
        };
        glyphs.push(glyph);
    }
    let [original, adjusted] = glyphs.as_slice() else {
        return Err("expected both path glyphs".into());
    };
    assert_eq!(original.text, "H");
    assert_eq!(adjusted.text, original.text);
    assert_eq!(adjusted.id, original.id);
    let initial = original.transform();
    let actual = adjusted.transform();
    assert!(initial.sx > 0.0);
    near(actual.sx, -initial.ky);
    near(actual.ky, initial.sx);
    near(actual.kx, -initial.sy);
    near(actual.sy, initial.kx);
    Ok(())
}

#[test]
fn path_text_preserves_surviving_glyph_positions_when_a_span_font_is_missing() -> TestResult<()> {
    let available = path_text_with_content(0, "ABC")?;
    let missing = path_text_with_content(
        0,
        "A<tspan font-family='UnavailableFixtureFamily'>B</tspan>C",
    )?;
    let expected: Vec<_> = path_glyphs(text(&available)?)
        .filter(|(value, _, _)| *value != "B")
        .collect();
    assert_eq!(
        expected
            .iter()
            .map(|(value, _, _)| *value)
            .collect::<Vec<_>>(),
        ["A", "C"]
    );
    assert_eq!(path_glyphs(text(&missing)?).collect::<Vec<_>>(), expected);
    Ok(())
}

fn path_glyphs(
    text: &usvg::Text,
) -> impl Iterator<Item = (&str, usvg::GlyphId, tiny_skia::Transform)> {
    text.layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .map(|glyph| (glyph.text.as_str(), glyph.id, glyph.transform()))
}
