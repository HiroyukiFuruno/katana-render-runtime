use super::super::super::{FromValue, SvgNode};
use super::{AId, parse_document, raw_attribute};
use crate::svg_engine::usvg::{AlignmentBaseline, DominantBaseline, FontStyle, LengthAdjust};
use rstest::rstest;

#[rstest]
#[case::alignment("alignment-baseline", AId::AlignmentBaseline, "middle")]
#[case::dominant("dominant-baseline", AId::DominantBaseline, "hanging")]
#[case::length("lengthAdjust", AId::LengthAdjust, "spacingAndGlyphs")]
#[case::style("font-style", AId::FontStyle, "italic")]
fn text_attributes_reject_unknown_values_after_raw_parse(
    #[case] name: &str,
    #[case] aid: AId,
    #[case] valid: &str,
) -> Result<(), String> {
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg'><text id='invalid' {name}='unknown'>A</text><text id='valid' {name}='{valid}'>B</text></svg>"
    );
    let document = parse_document(&source, None)?;
    assert_eq!(raw_attribute(&document, "invalid", aid)?, "unknown");
    assert_eq!(raw_attribute(&document, "valid", aid)?, valid);
    let invalid = document
        .element_by_id("invalid")
        .ok_or("invalid node missing")?;
    let valid = document
        .element_by_id("valid")
        .ok_or("valid node missing")?;
    assert_typed_values(&invalid, &valid, aid)?;
    Ok(())
}

fn assert_values<'a, 'input: 'a, T: FromValue<'a, 'input> + PartialEq + std::fmt::Debug>(
    invalid: &SvgNode<'a, 'input>,
    valid: &SvgNode<'a, 'input>,
    aid: AId,
    expected: T,
) {
    assert_eq!(invalid.try_attribute::<T>(aid), None);
    assert_eq!(valid.try_attribute::<T>(aid), Some(expected));
}

fn assert_typed_values<'a, 'input: 'a>(
    invalid: &SvgNode<'a, 'input>,
    valid: &SvgNode<'a, 'input>,
    aid: AId,
) -> Result<(), String> {
    match aid {
        AId::AlignmentBaseline => assert_values(invalid, valid, aid, AlignmentBaseline::Middle),
        AId::DominantBaseline => assert_values(invalid, valid, aid, DominantBaseline::Hanging),
        AId::LengthAdjust => assert_values(invalid, valid, aid, LengthAdjust::SpacingAndGlyphs),
        AId::FontStyle => assert_values(invalid, valid, aid, FontStyle::Italic),
        _ => return Err("unexpected attribute".to_owned()),
    }
    Ok(())
}
