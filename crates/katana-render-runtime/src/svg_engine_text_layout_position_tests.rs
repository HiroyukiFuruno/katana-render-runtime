use super::{near, parse, text, usvg};
use rstest::rstest;

const SHIFT_X: f32 = 7.0;
const SHIFT_Y: f32 = 5.0;
const INHERITED_ROTATIONS: &[f32] = &[10.0, 20.0, 20.0, 20.0];
const OVERRIDDEN_ROTATIONS: &[f32] = &[10.0, 20.0, 30.0, 30.0];

#[rstest]
#[case::inherited("<tspan>CD</tspan>", INHERITED_ROTATIONS)]
#[case::child_override("<tspan rotate='30'>CD</tspan>", OVERRIDDEN_ROTATIONS)]
fn nested_rotation_repeats_last_value_and_prefers_child(
    #[case] child: &str,
    #[case] expected: &[f32],
) -> Result<(), String> {
    let tree = parse("rotate='10 20'", &format!("AB{child}"))?;
    assert_eq!(text(&tree)?.rotate(), expected);
    Ok(())
}

#[rstest]
#[case::horizontal("", usvg::WritingMode::LeftToRight)]
#[case::vertical("writing-mode='vertical-rl'", usvg::WritingMode::TopToBottom)]
fn decorations_create_only_requested_layout_paths(
    #[case] writing: &str,
    #[case] expected_mode: usvg::WritingMode,
) -> Result<(), String> {
    for (decoration, expected) in [
        ("none", (false, false, false)),
        ("underline", (true, false, false)),
        ("overline", (false, true, false)),
        ("line-through", (false, false, true)),
    ] {
        let tree = parse(&format!("{writing} text-decoration='{decoration}'"), "H")?;
        let text = text(&tree)?;
        assert_eq!(text.writing_mode(), expected_mode);
        let span = text.layouted().first().ok_or("layout span missing")?;
        assert_decoration(span, expected);
    }
    Ok(())
}

fn assert_decoration(span: &usvg::layout::Span, expected: (bool, bool, bool)) {
    assert!(!span.positioned_glyphs.is_empty());
    assert_eq!(
        (
            span.underline.is_some(),
            span.overline.is_some(),
            span.line_through.is_some()
        ),
        expected
    );
    let path = span
        .underline
        .as_ref()
        .or(span.overline.as_ref())
        .or(span.line_through.as_ref());
    if let Some(path) = path {
        assert!(path.bounding_box().width() > 0.0);
        assert!(path.bounding_box().height() > 0.0);
    }
}

#[test]
fn rotation_and_relative_position_transform_a_single_glyph() -> Result<(), String> {
    let base = parse("", "H")?;
    let rotated = parse("rotate='90'", "H")?;
    let shifted = parse("dx='7' dy='5'", "H")?;
    let glyph = |tree| -> Result<_, String> {
        Ok(text(tree)?
            .layouted()
            .first()
            .ok_or("span missing")?
            .positioned_glyphs
            .first()
            .ok_or("glyph missing")?
            .transform())
    };
    let original = glyph(&base)?;
    let rotation = glyph(&rotated)?;
    let offset = glyph(&shifted)?;
    near(rotation.sx, -original.ky);
    near(rotation.ky, original.sx);
    near(offset.tx - original.tx, SHIFT_X);
    near(offset.ty - original.ty, SHIFT_Y);
    Ok(())
}
