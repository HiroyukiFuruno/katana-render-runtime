use super::{parse, text};
use rstest::rstest;

#[path = "svg_engine_text_layout_css_x_height_tests.rs"]
mod css_x_height_tests;

#[rstest]
#[case::normal("normal", 400.0, None)]
#[case::single("'wght' 700", 700.0, None)]
#[case::double_quoted("&quot;wght&quot; 700", 700.0, None)]
#[case::two_axes("'wght' 700, 'wdth' 85", 700.0, Some(85.0))]
#[case::short_tag("'bad' 100, 'wght' 700", 700.0, None)]
#[case::invalid_value("'wdth' invalid, 'wght' 700", 700.0, None)]
#[case::unquoted_tag("wdth 85, 'wght' 700", 700.0, None)]
#[case::empty_component(", 'wght' 700, ", 700.0, None)]
#[case::mixed_axes("'bad' 10, 'wght' 700, 'wdth' 85", 700.0, Some(85.0))]
fn font_variations_preserve_explicit_axes(
    #[case] settings: &str,
    #[case] weight: f32,
    #[case] width: Option<f32>,
) -> Result<(), String> {
    let tree = parse(&format!("font-variation-settings=\"{settings}\""), "H")?;
    let span = text(&tree)?
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .ok_or("span missing")?;
    let axes = span.font().variations();
    assert_eq!(
        axes.iter()
            .find(|axis| &axis.tag == b"wght")
            .map(|axis| axis.value),
        Some(weight)
    );
    assert_eq!(
        axes.iter()
            .find(|axis| &axis.tag == b"wdth")
            .map(|axis| axis.value),
        width
    );
    Ok(())
}

#[test]
fn explicit_four_variation_axes_override_standard_style_axes() -> Result<(), String> {
    let tree = parse(
        "font-style='oblique' font-variation-settings=\"'wght' 650, 'wdth' 80, 'ital' 1, 'slnt' -8\"",
        "H",
    )?;
    let span = text(&tree)?
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .ok_or("span missing")?;
    let axes = span.font().variations();
    for (tag, value) in [
        (b"wght", 650.0),
        (b"wdth", 80.0),
        (b"ital", 1.0),
        (b"slnt", -8.0),
    ] {
        let matching: Vec<_> = axes.iter().filter(|axis| &axis.tag == tag).collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].value, value);
    }
    Ok(())
}

#[test]
fn invalid_baseline_shift_is_baseline_while_percentage_uses_font_size() -> Result<(), String> {
    let tree = parse(
        "",
        "<tspan baseline-shift='invalid'>A</tspan><tspan baseline-shift='25%'>B</tspan>",
    )?;
    let spans = text(&tree)?
        .chunks()
        .first()
        .ok_or("chunk missing")?
        .spans();
    assert_eq!(spans.len(), 2);
    assert!(spans[0].baseline_shift().is_empty());
    assert_eq!(
        spans[1].baseline_shift(),
        &[super::usvg::BaselineShift::Number(5.0)]
    );
    Ok(())
}
