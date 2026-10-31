use super::{text_node, upstream_writer_font_database, usvg};
use rstest::rstest;

const OFFSET_TOLERANCE: f32 = 0.001;

fn parse_svg(input: &str) -> Result<usvg::Tree, String> {
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(input, &options).map_err(|error| error.to_string())
}

#[rstest]
#[case::quadratic("M0 0 Q10 0 20 0", 10.0)]
#[case::closed("M0 0 L20 0 Z", 20.0)]
fn text_path_percentage_uses_entire_path_length(
    #[case] path: &str,
    #[case] expected: f32,
) -> Result<(), String> {
    let input = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><defs><path id='curve' d='{path}'/></defs><text id='target' font-family='Yellowtail'><textPath href='#curve' startOffset='50%'>A</textPath></text></svg>"
    );
    let tree = parse_svg(&input)?;
    let chunk = text_node(&tree, "target")?
        .chunks()
        .first()
        .ok_or("chunk missing")?;
    let usvg::TextFlow::Path(flow) = chunk.text_flow() else {
        return Err("textPath flow missing".to_owned());
    };
    assert!((flow.start_offset() - expected).abs() < OFFSET_TOLERANCE);
    Ok(())
}

#[rstest]
#[case::resolved("#source", "ABX")]
#[case::missing("#missing", "X")]
fn tref_collects_referenced_text_and_preserves_following_text(
    #[case] href: &str,
    #[case] expected: &str,
) -> Result<(), String> {
    let input = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><defs><text id='source'>A<tspan>B</tspan></text></defs><text id='target' font-family='Yellowtail'><tref href='{href}'/>X</text></svg>"
    );
    let tree = parse_svg(&input)?;
    let text = text_node(&tree, "target")?;
    let actual: String = text.chunks().iter().map(|chunk| chunk.text()).collect();
    assert_eq!(actual, expected);
    Ok(())
}
