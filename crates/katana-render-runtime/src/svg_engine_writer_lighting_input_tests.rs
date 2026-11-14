use super::super::{Pixmap, usvg};
use super::{parse_writer_svg, render_tree, writer_options};

const VIEWPORT: u32 = 64;
const RECT_ORIGIN: u32 = 8;
const RECT_SIZE: u32 = 48;
const RGBA_CHANNELS: usize = 4;
const ALPHA_CHANNEL: usize = 3;
const TRANSPARENT_ALPHA: u8 = 0;

#[test]
fn lighting_inputs_survive_serialization_and_affect_rendering() -> Result<(), String> {
    for primitive in ["feDiffuseLighting", "feSpecularLighting"] {
        assert_lighting_inputs(primitive)?;
    }
    Ok(())
}

fn assert_lighting_inputs(primitive: &str) -> Result<(), String> {
    let source_alpha = render_lighting_input(primitive, "SourceAlpha")?;
    let reference = render_lighting_input(primitive, "transparent")?;
    assert!(
        has_visible_pixel(source_alpha.data()),
        "{primitive}: SourceAlpha empty"
    );
    assert_ne!(
        source_alpha.data(),
        reference.data(),
        "{primitive}: input ignored"
    );
    Ok(())
}

fn render_lighting_input(primitive: &str, input: &str) -> Result<Pixmap, String> {
    let case = format!("{primitive}-{input}");
    let svg = lighting_svg(primitive, input);
    let original = parse_writer_svg(&svg, &case)?;
    let expected = expected_input(input);
    assert_input(&original, &expected, &case);
    let serialized = original.to_string(&writer_options(&case));
    assert!(
        serialized.contains(&format!("<{primitive}")),
        "{case}: primitive absent"
    );
    assert!(
        serialized.contains(&format!("in=\"{input}\"")),
        "{case}: input absent"
    );
    let reparsed = parse_writer_svg(&serialized, &case)?;
    assert_input(&reparsed, &expected, &case);
    let before = render_tree(&original)?;
    let after = render_tree(&reparsed)?;
    assert_eq!(before.data(), after.data(), "{case}: pixels changed");
    render_tree(&original)
}

fn lighting_svg(primitive: &str, input: &str) -> String {
    let light = if primitive == "feDiffuseLighting" {
        "surfaceScale=\"1\" diffuseConstant=\"1\""
    } else {
        "surfaceScale=\"1\" specularConstant=\"1\" specularExponent=\"1\""
    };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{VIEWPORT}\" height=\"{VIEWPORT}\"><defs><filter id=\"f\"><feFlood flood-color=\"white\" flood-opacity=\"0\" result=\"transparent\"/><{primitive} in=\"{input}\" {light}><feDistantLight azimuth=\"45\" elevation=\"45\"/></{primitive}></filter></defs><rect x=\"{RECT_ORIGIN}\" y=\"{RECT_ORIGIN}\" width=\"{RECT_SIZE}\" height=\"{RECT_SIZE}\" fill=\"red\" filter=\"url(#f)\"/></svg>"
    )
}

fn expected_input(input: &str) -> usvg::filter::Input {
    match input {
        "SourceAlpha" => usvg::filter::Input::SourceAlpha,
        _ => usvg::filter::Input::Reference(input.to_owned()),
    }
}

fn assert_input(tree: &usvg::Tree, expected: &usvg::filter::Input, case: &str) {
    assert!(
        tree.filters()
            .iter()
            .flat_map(|filter| filter.primitives())
            .any(|primitive| primitive.kind().has_input(expected)),
        "{case}: parsed input differs"
    );
}

fn has_visible_pixel(pixels: &[u8]) -> bool {
    pixels
        .chunks_exact(RGBA_CHANNELS)
        .any(|pixel| pixel[ALPHA_CHANNEL] > TRANSPARENT_ALPHA)
}
