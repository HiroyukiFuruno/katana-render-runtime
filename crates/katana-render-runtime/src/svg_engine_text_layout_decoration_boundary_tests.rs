use super::{FONT, TestResult, database_with_font, parse, text, usvg};
use rstest::rstest;
use tiny_skia::Transform;
use usvg::layout::{DatabaseExt, DecorationSpan, ResolvedFont, convert_decoration};

const VALID_DECORATION_WIDTH: f32 = 10.0;

fn fixture() -> TestResult<(usvg::TextSpan, ResolvedFont)> {
    let tree = parse("text-decoration='underline'", "H")?;
    let span = text(&tree)?
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .cloned()
        .ok_or("text span missing")?;
    let (database, id) = database_with_font(FONT.to_vec())?;
    let font = database.load_font(id, &[]).ok_or("font metrics missing")?;
    Ok((span, font))
}

#[rstest]
#[case::zero(0.0, false)]
#[case::negative(-1.0, false)]
#[case::nan(f32::NAN, false)]
#[case::infinite(f32::INFINITY, false)]
#[case::invalid_transform(VALID_DECORATION_WIDTH, true)]
fn malformed_decoration_spans_do_not_hide_valid_spans(
    #[case] width: f32,
    #[case] invalid_transform: bool,
) -> TestResult<()> {
    let (span, font) = fixture()?;
    let invalid = DecorationSpan {
        width,
        transform: if invalid_transform {
            Transform::from_translate(f32::NAN, 0.0)
        } else {
            Transform::identity()
        },
    };
    assert_decoration_isolated(&span, &font, invalid)
}

fn assert_decoration_isolated(
    span: &usvg::TextSpan,
    font: &ResolvedFont,
    invalid: DecorationSpan,
) -> TestResult<()> {
    let style = span
        .decoration()
        .underline()
        .cloned()
        .ok_or("underline missing")?;
    let valid = DecorationSpan {
        width: VALID_DECORATION_WIDTH,
        transform: Transform::identity(),
    };
    let convert = |spans: &[DecorationSpan]| {
        convert_decoration(0.0, span, font, style.clone(), spans, Transform::identity())
    };
    let expected = convert(&[valid]).ok_or("valid decoration missing")?;
    assert!(expected.is_visible() && expected.fill().is_some());
    assert!(convert(&[invalid]).is_none());
    let actual = convert(&[invalid, valid]).ok_or("valid decoration was discarded")?;
    assert_eq!(actual.bounding_box(), expected.bounding_box());
    assert!(actual.is_visible() && actual.fill().is_some());
    Ok(())
}
