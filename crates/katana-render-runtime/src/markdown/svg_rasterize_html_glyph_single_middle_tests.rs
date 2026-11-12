use super::super::{HtmlGlyphRasterizer, Prepared, PreparedSpan, paint};
use resvg::usvg::{Node, Options, Text, Tree};

const CSS_SCALE: f32 = 0.999_990_4;
const MIDDLE_CASE_COUNT: usize = 5;

#[derive(Clone, Copy)]
struct Case {
    text: &'static str,
    anchor_x: f32,
    font_size: f32,
    weight: u16,
    italic: bool,
    expected_x: f64,
}

const CASES: [Case; MIDDLE_CASE_COUNT] = [
    Case {
        text: "API Application",
        anchor_x: 466.0,
        font_size: 16.0,
        weight: 700,
        italic: false,
        expected_x: 407.203_125,
    },
    Case {
        text: "[CONTAINER]",
        anchor_x: 466.0,
        font_size: 14.0,
        weight: 400,
        italic: false,
        expected_x: 421.421_875,
    },
    Case {
        text: "Uses",
        anchor_x: 296.501_16,
        font_size: 12.0,
        weight: 400,
        italic: false,
        expected_x: 282.828_125,
    },
    Case {
        text: "[JSON/HTTPS]",
        anchor_x: 320.501_16,
        font_size: 12.0,
        weight: 400,
        italic: true,
        expected_x: 279.843_75,
    },
    Case {
        text: "Calls",
        anchor_x: 479.5,
        font_size: 12.0,
        weight: 400,
        italic: false,
        expected_x: 466.156_25,
    },
];

#[test]
fn single_middle_origins_match_all_five_observed_chrome_runs() -> Result<(), String> {
    for case in CASES {
        let source = tree(&case, "middle", "")?;
        let spans = prepared(&source)?;
        let [span] = spans.as_slice() else {
            return Err("single middle span count mismatch".to_owned());
        };
        let glyph = span
            .glyphs
            .first()
            .ok_or("single middle glyph is missing")?;
        assert!(span.x_alignment.is_none());
        assert!(span.chunk_alignment.is_some());
        for device_ratio in [1.0, 2.0] {
            let actual = paint::normalized_transform(span, glyph, CSS_SCALE * device_ratio);
            assert_eq!(
                f64::from(actual.tx / device_ratio),
                case.expected_x,
                "{}",
                case.text
            );
        }
    }
    Ok(())
}

#[test]
fn start_end_and_one_glyph_keep_the_previous_alignment_lane() -> Result<(), String> {
    for anchor in ["start", "end"] {
        let source = tree(&CASES[0], anchor, "")?;
        let spans = prepared(&source)?;
        assert!(
            spans
                .iter()
                .all(|span| span.chunk_alignment.is_none() && span.x_alignment.is_some())
        );
    }
    let one = Case {
        text: "A",
        ..CASES[0]
    };
    let source = tree(&one, "middle", "")?;
    let spans = prepared(&source)?;
    assert!(spans.iter().all(|span| {
        span.chunk_alignment.is_none()
            && span
                .x_alignment
                .is_some_and(|alignment| alignment.layout.is_some())
    }));
    Ok(())
}

#[test]
fn shifted_source_anchor_rejects_the_forward_origin_contract() -> Result<(), String> {
    let source = tree(&CASES[0], "middle", "")?;
    let mut spans = prepared(&source)?;
    assert_eq!(spans.len(), 1);
    let span = &mut spans[0];
    span.chunk_alignment = None;
    assert!(super::eligibility::single_middle_input(text(&source)?, span).is_some());
    let shifted = Case {
        anchor_x: CASES[0].anchor_x + 1.0,
        ..CASES[0]
    };
    let shifted_source = tree(&shifted, "middle", "")?;
    assert!(super::eligibility::single_middle_input(text(&shifted_source)?, span).is_none());
    Ok(())
}

#[test]
fn single_middle_rejects_y_origin_beyond_layout_unit_range() -> Result<(), String> {
    let source = tree(&CASES[0], "middle", "dy='40000000'")?;
    let text = text(&source)?;
    assert_eq!(text.dy().first(), Some(&40_000_000.0));
    let result = HtmlGlyphRasterizer::new(source.fontdb()).prepare(text);
    assert!(matches!(result, Err(error) if error == "chunk Y origin exceeds LayoutUnit"));
    Ok(())
}

#[test]
fn explicit_offsets_spacing_and_nonstandard_features_remain_ineligible() -> Result<(), String> {
    for attributes in [
        "dx='1'",
        "dy='0 1'",
        "rotate='3'",
        "letter-spacing='1'",
        "word-spacing='1'",
        "textLength='180'",
        "style='font-kerning:none'",
        "font-variation-settings=\"'wght' 600\"",
    ] {
        let source = tree(&CASES[0], "middle", attributes)?;
        if let Prepared::Ready(spans) = prepare_tree(&source)? {
            assert!(
                spans.iter().all(|span| span.chunk_alignment.is_none()),
                "{attributes}"
            );
        }
    }
    Ok(())
}

fn prepared(source: &Tree) -> Result<Vec<PreparedSpan>, String> {
    match prepare_tree(source)? {
        Prepared::Ready(spans) => Ok(spans),
        Prepared::Unsupported => Err("single middle source is unsupported".to_owned()),
    }
}

fn prepare_tree(source: &Tree) -> Result<Prepared, String> {
    HtmlGlyphRasterizer::new(source.fontdb()).prepare(text(source)?)
}

fn text(source: &Tree) -> Result<&Text, String> {
    match source.node_by_id("run") {
        Some(Node::Text(text)) => Ok(text),
        _ => Err("single middle source text is missing".to_owned()),
    }
}

fn tree(case: &Case, anchor: &str, attributes: &str) -> Result<Tree, String> {
    let style = if case.italic { "italic" } else { "normal" };
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg'><text id='run' x='{}' y='14' \
         font-family='Helvetica' font-size='{}' font-weight='{}' font-style='{style}' \
         text-anchor='{anchor}' transform='scale({CSS_SCALE})' {attributes}>{}</text></svg>",
        case.anchor_x, case.font_size, case.weight, case.text,
    );
    let options = Options {
        fontdb: crate::markdown::svg_rasterize::font::html_font_db_for_text("Helvetica", case.text),
        ..Options::default()
    };
    Tree::from_str(&svg, &options).map_err(|error| error.to_string())
}
