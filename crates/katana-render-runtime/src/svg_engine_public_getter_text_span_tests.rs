use super::{assert_target_text, parse_svg, text_node};
use rstest::rstest;

#[rstest]
#[case::default("default", " Aé ", "Aé")]
#[case::preserved("preserve", " Aé ", " Aé ")]
#[case::single_character("default", "A", "A")]
fn single_svg_text_node_applies_xml_space_without_changing_characters(
    #[case] space: &str,
    #[case] content: &str,
    #[case] expected: &str,
) -> Result<(), String> {
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><text id='target' xml:space='{space}' font-family='Yellowtail'>{content}</text></svg>"
    );
    assert_target_text(&source, expected)
}

#[test]
fn positioned_text_chunks_reset_span_byte_offsets_for_multibyte_text() -> Result<(), String> {
    let input = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><text id='target' x='10' y='20' font-family='Yellowtail'>Aé<tspan fill='red'>Bø</tspan><tspan x='40'>Cü</tspan>Dñ</text></svg>";
    let tree = parse_svg(input)?;
    let chunks = text_node(&tree, "target")?.chunks();
    assert_eq!(chunks.len(), 2);
    for (chunk, (x, parts)) in chunks
        .iter()
        .zip([(10.0, ["Aé", "Bø"]), (40.0, ["Cü", "Dñ"])])
    {
        assert_eq!(chunk.x(), Some(x));
        assert_eq!(chunk.text(), parts.concat());
        assert_eq!(chunk.spans().len(), parts.len());
        let mut start = 0;
        for (span, expected) in chunk.spans().iter().zip(parts) {
            let end = start + expected.len();
            assert_eq!((span.start(), span.end()), (start, end));
            assert_eq!(chunk.text().get(start..end), Some(expected));
            start = end;
        }
    }
    Ok(())
}
