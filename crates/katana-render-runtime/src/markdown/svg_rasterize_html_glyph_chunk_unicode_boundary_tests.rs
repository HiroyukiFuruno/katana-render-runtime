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
    let [first, second] = spans else {
        return Err("expected two SVG spans".into());
    };
    assert!(accepts_span_ranges(chunk.text(), &[first, second], false));
    assert!(!accepts_span_ranges(chunk.text(), &[first], false));
    assert!(!accepts_span_ranges(chunk.text(), &[second, first], false));
    assert!(!accepts_span_ranges("A", &[first, second], false));
    assert!(!accepts_span_ranges(chunk.text(), &[first, second], true));
    Ok(())
}

fn accepts_span_ranges(text: &str, spans: &[&usvg::TextSpan], ascii_only: bool) -> bool {
    let mut matched = Vec::new();
    let spans = spans.iter().copied().zip(std::iter::repeat(()));
    let accepted = ranges_match(text, spans, |_, content, ()| {
        matched.push(content.to_owned());
        !ascii_only || content.is_ascii()
    });
    if ascii_only {
        assert_eq!(matched, ["é"]);
    }
    accepted
}
