use super::{fixture, usvg};
use rstest::rstest;

#[rstest]
#[case::first_omitted("<feOffset/>", usvg::filter::Input::SourceGraphic)]
#[case::explicit_source_alpha("<feOffset in='SourceAlpha'/>", usvg::filter::Input::SourceAlpha)]
#[case::later_omitted(
    "<feFlood result='prior'/><feOffset/>",
    usvg::filter::Input::Reference("prior".to_owned())
)]
fn offset_uses_svg_filter_input_defaults(
    #[case] children: &str,
    #[case] expected: usvg::filter::Input,
) -> Result<(), String> {
    let tree = fixture(children)?;
    let offset = tree
        .filters()
        .first()
        .and_then(|filter| filter.primitives().last())
        .ok_or("offset primitive missing")?;
    let usvg::filter::Kind::Offset(offset) = offset.kind() else {
        return Err("expected feOffset".to_owned());
    };
    assert_eq!(offset.input(), &expected);
    Ok(())
}
