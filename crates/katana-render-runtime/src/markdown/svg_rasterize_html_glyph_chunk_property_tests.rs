use super::{CSS_SCALE, SPLIT_CONTENT, assert_excluded, tree};
use resvg::usvg::{Node, Text, TextSpan};

type RetainedProperty = fn(&Text) -> bool;

const EXPLICIT_PROPERTY_CASE_COUNT: usize = 8;
const CASES: [(&str, RetainedProperty); EXPLICIT_PROPERTY_CASE_COUNT] = [
    ("dx='1'", has_dx),
    ("dy='0 1'", has_later_dy),
    ("rotate='3'", has_rotation),
    ("letter-spacing='1'", has_letter_spacing),
    ("word-spacing='1'", has_word_spacing),
    ("style='font-kerning:none'", has_disabled_kerning),
    ("font-variation-settings=\"'wght' 600\"", has_custom_weight),
    ("font-variation-settings=\"'opsz' 14\"", has_optical_axis),
];

#[test]
fn retained_explicit_properties_exclude_chunk_alignment() -> Result<(), String> {
    for (attributes, retained) in CASES {
        let source = tree(SPLIT_CONTENT, attributes, CSS_SCALE)?;
        let Some(Node::Text(text)) = source.node_by_id("run") else {
            return Err(format!(
                "explicit property source text is missing: {attributes}"
            ));
        };
        assert!(
            retained(text),
            "property was not retained by usvg: {attributes}"
        );
        assert_excluded(SPLIT_CONTENT, attributes)?;
    }
    Ok(())
}

#[test]
fn presentation_font_kerning_is_discarded_by_the_usvg_model() -> Result<(), String> {
    let source = tree(SPLIT_CONTENT, "font-kerning='none'", CSS_SCALE)?;
    let Some(Node::Text(text)) = source.node_by_id("run") else {
        return Err("presentation font-kerning source text is missing".to_owned());
    };
    /* WHY: usvgはfont-kerningをCSSでのみ取り込み、presentation属性は明示的に捨てる。 */
    assert!(!has_disabled_kerning(text));
    Ok(())
}

fn any_span(text: &Text, predicate: impl FnMut(&TextSpan) -> bool) -> bool {
    text.chunks()
        .iter()
        .flat_map(|chunk| chunk.spans())
        .any(predicate)
}

fn has_dx(text: &Text) -> bool {
    text.dx().iter().any(|value| *value != 0.0)
}

fn has_later_dy(text: &Text) -> bool {
    text.dy().iter().skip(1).any(|value| *value != 0.0)
}

fn has_rotation(text: &Text) -> bool {
    text.rotate().iter().any(|value| *value != 0.0)
}

fn has_letter_spacing(text: &Text) -> bool {
    any_span(text, |span| span.letter_spacing() != 0.0)
}

fn has_word_spacing(text: &Text) -> bool {
    any_span(text, |span| span.word_spacing() != 0.0)
}

fn has_disabled_kerning(text: &Text) -> bool {
    any_span(text, |span| !span.apply_kerning())
}

fn has_custom_weight(text: &Text) -> bool {
    any_span(text, |span| {
        span.font()
            .variations()
            .iter()
            .any(|axis| axis.tag == *b"wght" && axis.value != f32::from(span.font().weight()))
    })
}

fn has_optical_axis(text: &Text) -> bool {
    any_span(text, |span| {
        span.font()
            .variations()
            .iter()
            .any(|axis| axis.tag == *b"opsz")
    })
}
