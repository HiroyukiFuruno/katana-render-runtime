use rstest::rstest;
use std::path::Path;

use super::{
    ALPHA_CHANNEL, RGBA_CHANNELS, first_text, parse_writer_svg, usvg,
    writer_coverage_tests::assert_pixels_equal,
};

const TEXT_PATH_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/text/textPath/startOffset=30.svg"
);

#[test]
fn nonzero_text_path_offset_survives_preserving_writer() -> Result<(), String> {
    let input =
        std::fs::read_to_string(Path::new(TEXT_PATH_FIXTURE)).map_err(|error| error.to_string())?;
    let original = parse_writer_svg(&input, "text-path-start-offset-30")?;
    let options = usvg::WriteOptions {
        preserve_text: true,
        ..usvg::WriteOptions::default()
    };
    let serialized = original.to_string(&options);
    assert_serialized_attribute(&serialized, "startOffset", "30")?;
    let reparsed = parse_writer_svg(&serialized, "text-path-start-offset-30")?;
    assert_pixels_equal(&original, &reparsed, "text path start offset")?;
    Ok(())
}

#[rstest]
#[case::double_attribute_quotes(false, "'Odd/Face', Noto Sans")]
#[case::single_attribute_quotes(true, "\"Odd/Face\", Noto Sans")]
fn quoted_font_family_survives_preserving_writer_roundtrip(
    #[case] use_single_quote: bool,
    #[case] expected_family: &str,
) -> Result<(), String> {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60"><text x="12" y="32" font-family="'Odd/Face', Noto Sans" font-size="18">Text</text></svg>"#;
    let original = parse_writer_svg(input, "writer-quoted-font-family")?;
    let options = usvg::WriteOptions {
        preserve_text: true,
        use_single_quote,
        ..usvg::WriteOptions::default()
    };
    let serialized = original.to_string(&options);
    assert_tspan_attribute(&serialized, "font-family", expected_family)?;
    let reparsed = parse_writer_svg(&serialized, "writer-quoted-font-family")?;
    assert_pixels_equal(&original, &reparsed, "quoted font family")?;
    Ok(())
}

#[test]
fn dominant_no_change_inherits_parent_before_writer_roundtrip() -> Result<(), String> {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><text x="50" y="50" font-family="Noto Sans" font-size="18" dominant-baseline="middle"><tspan dominant-baseline="no-change">Edge Text</tspan></text></svg>"#;
    let original = parse_writer_svg(input, "dominant-no-change-inherits-parent")?;
    assert_dominant_baseline(&original, usvg::DominantBaseline::Middle)?;
    let options = usvg::WriteOptions {
        preserve_text: true,
        ..usvg::WriteOptions::default()
    };
    let serialized = original.to_string(&options);
    assert_tspan_attribute(&serialized, "dominant-baseline", "middle")?;
    let reparsed = parse_writer_svg(&serialized, "dominant-no-change-inherits-parent")?;
    assert_dominant_baseline(&reparsed, usvg::DominantBaseline::Middle)?;
    assert_pixels_equal(&original, &reparsed, "dominant no-change inheritance")?;
    Ok(())
}

fn assert_dominant_baseline(
    tree: &usvg::Tree,
    expected: usvg::DominantBaseline,
) -> Result<(), String> {
    let text = first_text(tree.root()).ok_or("text node missing")?;
    let actual = text
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .map(usvg::TextSpan::dominant_baseline)
        .ok_or("text span missing")?;
    assert_eq!(actual, expected);
    Ok(())
}

fn assert_tspan_attribute(svg: &str, name: &str, expected: &str) -> Result<(), String> {
    let document = usvg::roxmltree::Document::parse(svg).map_err(|error| error.to_string())?;
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("tspan") && node.attribute(name) == Some(expected) })
    );
    Ok(())
}

#[rstest]
#[case::monospace("font-family", "monospace", "font-family", "monospace")]
#[case::serif("font-family", "serif", "font-family", "serif")]
#[case::sans_serif("font-family", "sans-serif", "font-family", "sans-serif")]
#[case::cursive("font-family", "cursive", "font-family", "cursive")]
#[case::fantasy("font-family", "fantasy", "font-family", "fantasy")]
#[case::italic("font-style", "italic", "font-style", "italic")]
#[case::oblique("font-style", "oblique", "font-style", "oblique")]
#[case::condensed("font-stretch", "condensed", "font-stretch", "condensed")]
#[case::extra_condensed("font-stretch", "extra-condensed", "font-stretch", "extra-condensed")]
#[case::ultra_condensed("font-stretch", "ultra-condensed", "font-stretch", "ultra-condensed")]
#[case::semi_condensed("font-stretch", "semi-condensed", "font-stretch", "semi-condensed")]
#[case::expanded("font-stretch", "expanded", "font-stretch", "expanded")]
#[case::semi_expanded("font-stretch", "semi-expanded", "font-stretch", "semi-expanded")]
#[case::extra_expanded("font-stretch", "extra-expanded", "font-stretch", "extra-expanded")]
#[case::ultra_expanded("font-stretch", "ultra-expanded", "font-stretch", "ultra-expanded")]
#[case::letter_spacing("letter-spacing", "2", "letter-spacing", "2")]
#[case::word_spacing("word-spacing", "3", "word-spacing", "3")]
#[case::small_caps("font-variant", "small-caps", "font-variant", "small-caps")]
#[case::baseline_shift_number("baseline-shift", "3", "baseline-shift", "3")]
#[case::baseline_shift_subscript("baseline-shift", "sub", "baseline-shift", "sub")]
#[case::no_kerning("font-kerning", "none", "style", "font-kerning:none")]
#[case::paint_order("paint-order", "stroke fill", "paint-order", "stroke fill")]
#[case::writing_mode("writing-mode", "tb", "writing-mode", "tb")]
#[case::anchor_middle("text-anchor", "middle", "text-anchor", "middle")]
#[case::anchor_end("text-anchor", "end", "text-anchor", "end")]
#[case::render_speed("text-rendering", "optimizeSpeed", "text-rendering", "optimizeSpeed")]
#[case::render_precision(
    "text-rendering",
    "geometricPrecision",
    "text-rendering",
    "geometricPrecision"
)]
#[case::dominant_before(
    "dominant-baseline",
    "text-before-edge",
    "dominant-baseline",
    "text-before-edge"
)]
#[case::dominant_after(
    "dominant-baseline",
    "text-after-edge",
    "dominant-baseline",
    "text-after-edge"
)]
#[case::dominant_ideographic(
    "dominant-baseline",
    "ideographic",
    "dominant-baseline",
    "ideographic"
)]
#[case::dominant_use_script("dominant-baseline", "use-script", "dominant-baseline", "use-script")]
#[case::dominant_reset_size("dominant-baseline", "reset-size", "dominant-baseline", "reset-size")]
#[case::dominant_alphabetic("dominant-baseline", "alphabetic", "dominant-baseline", "alphabetic")]
#[case::alignment_before(
    "alignment-baseline",
    "text-before-edge",
    "alignment-baseline",
    "text-before-edge"
)]
#[case::alignment_central("alignment-baseline", "central", "alignment-baseline", "central")]
#[case::alignment_middle("alignment-baseline", "middle", "alignment-baseline", "middle")]
#[case::alignment_after(
    "alignment-baseline",
    "text-after-edge",
    "alignment-baseline",
    "text-after-edge"
)]
#[case::alignment_hanging("alignment-baseline", "hanging", "alignment-baseline", "hanging")]
#[case::alignment_math(
    "alignment-baseline",
    "mathematical",
    "alignment-baseline",
    "mathematical"
)]
fn text_style_survives_preserving_writer(
    #[case] property: &str,
    #[case] value: &str,
    #[case] output_attribute: &str,
    #[case] output_value: &str,
) -> Result<(), String> {
    let input = style_input(property, value);
    let original = parse_writer_svg(&input, property)?;
    let options = usvg::WriteOptions {
        preserve_text: true,
        ..usvg::WriteOptions::default()
    };
    let serialized = original.to_string(&options);
    assert_serialized_attribute(&serialized, output_attribute, output_value)?;
    let reparsed = parse_writer_svg(&serialized, property)?;
    assert_text_model(&original, &reparsed)?;
    let rendered = assert_pixels_equal(&original, &reparsed, property)?;
    assert!(
        rendered
            .data()
            .chunks_exact(RGBA_CHANNELS)
            .any(|pixel| pixel[ALPHA_CHANNEL] != 0)
    );
    Ok(())
}

fn style_input(property: &str, value: &str) -> String {
    if property == "baseline-shift" {
        return format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><text x="50" y="50" font-family="Noto Sans" font-size="18"><tspan style="{property}:{value}">Edge Text</tspan></text></svg>"#
        );
    }
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><text x="50" y="50" font-family="Noto Sans" font-size="18" style="{property}:{value}">Edge Text</text></svg>"#
    )
}

fn assert_serialized_attribute(svg: &str, name: &str, expected: &str) -> Result<(), String> {
    let document = usvg::roxmltree::Document::parse(svg).map_err(|error| error.to_string())?;
    assert!(
        document
            .descendants()
            .any(|node| node.attribute(name) == Some(expected)),
        "writer omitted {name}={expected}"
    );
    Ok(())
}

fn assert_text_model(original: &usvg::Tree, parsed: &usvg::Tree) -> Result<(), String> {
    let left = first_text(original.root()).ok_or("original text missing")?;
    let right = first_text(parsed.root()).ok_or("reparsed text missing")?;
    assert_eq!(left.writing_mode(), right.writing_mode());
    assert_eq!(left.rendering_mode(), right.rendering_mode());
    assert_eq!(left.chunks().len(), right.chunks().len());
    for (a, b) in left.chunks().iter().zip(right.chunks()) {
        assert_eq!(a.text(), b.text());
        assert_eq!(a.anchor(), b.anchor());
        assert_eq!(a.spans().len(), b.spans().len());
        for (first, second) in a.spans().iter().zip(b.spans()) {
            assert_span_model(first, second);
        }
    }
    Ok(())
}

fn assert_span_model(first: &usvg::TextSpan, second: &usvg::TextSpan) {
    assert_eq!(first.font(), second.font());
    assert_eq!(first.small_caps(), second.small_caps());
    assert_eq!(first.apply_kerning(), second.apply_kerning());
    assert_eq!(first.paint_order(), second.paint_order());
    assert_eq!(first.letter_spacing(), second.letter_spacing());
    assert_eq!(first.word_spacing(), second.word_spacing());
    assert_eq!(first.dominant_baseline(), second.dominant_baseline());
    assert_eq!(first.alignment_baseline(), second.alignment_baseline());
}
