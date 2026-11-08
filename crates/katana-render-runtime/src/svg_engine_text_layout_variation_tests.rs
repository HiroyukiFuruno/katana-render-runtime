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
