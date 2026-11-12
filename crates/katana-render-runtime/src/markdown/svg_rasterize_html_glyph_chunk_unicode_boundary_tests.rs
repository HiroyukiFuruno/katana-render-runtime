use super::{ranges_match, span_content};
use resvg::usvg;

#[test]
fn utf8_span_ranges_require_nonempty_character_boundaries() {
    let text = "é A";
    assert_eq!(span_content(text, 0, 2), Some("é"));
    assert_eq!(span_content(text, 2, text.len()), Some(" A"));
    assert!(span_content(text, 0, 1).is_none());
    assert!(span_content(text, 1, 2).is_none());
    assert!(span_content(text, 2, 2).is_none());
    assert!(span_content(text, 0, text.len() + 1).is_none());
}

#[test]
fn real_svg_span_ranges_reject_reordering_and_mismatched_text() -> Result<(), String> {
    let database = crate::markdown::svg_rasterize::font::bundled_font_db();
    let options = usvg::Options {
        fontdb: database,
        ..Default::default()
    };
    let svg = "<svg xmlns='http://www.w3.org/2000/svg'><text id='t'><tspan>é</tspan><tspan font-weight='700'>A</tspan></text></svg>";
    let tree = usvg::Tree::from_str(svg, &options).map_err(|error| error.to_string())?;
    let Some(usvg::Node::Text(text)) = tree.node_by_id("t") else {
        return Err("SVG text missing".into());
    };
    let chunk = text.chunks().first().ok_or("SVG chunk missing")?;
    let spans = chunk.spans();
    assert_eq!(spans.len(), 2);
    assert!(accepts_span_ranges(chunk.text(), spans.iter()));
    assert!(!accepts_span_ranges(chunk.text(), spans.iter().rev()));
    assert!(!accepts_span_ranges("A", spans.iter()));
    Ok(())
}

fn accepts_span_ranges<'a>(text: &str, spans: impl Iterator<Item = &'a usvg::TextSpan>) -> bool {
    ranges_match(text, spans.zip(std::iter::repeat(())), |_, _, ()| true)
}
