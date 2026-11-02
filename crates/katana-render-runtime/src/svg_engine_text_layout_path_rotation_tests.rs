use super::{near, path_support::path_text, text, usvg};

#[test]
fn rotation_is_applied_to_a_glyph_following_a_horizontal_path() -> Result<(), String> {
    let base = path_text(0)?;
    let rotated = path_text(90)?;
    let mut glyphs = Vec::new();
    for tree in [&base, &rotated] {
        let node = text(tree)?;
        let chunk = node.chunks().first().ok_or("chunk missing")?;
        assert!(matches!(chunk.text_flow(), usvg::TextFlow::Path(_)));
        let span = node.layouted().first().ok_or("layout span missing")?;
        let [glyph] = span.positioned_glyphs.as_slice() else {
            return Err("expected one path glyph".to_owned());
        };
        glyphs.push(glyph);
    }
    let [original, adjusted] = glyphs.as_slice() else {
        return Err("expected both path glyphs".to_owned());
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
