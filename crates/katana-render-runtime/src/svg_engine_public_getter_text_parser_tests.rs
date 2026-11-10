use super::{text_node, upstream_writer_font_database, usvg};
use rstest::rstest;

const OFFSET_TOLERANCE: f32 = 0.001;
const PATH_POINT_COUNT: usize = 2;
const PLAIN_PATH_POINTS: [(f32, f32); PATH_POINT_COUNT] = [(0.0, 0.0), (20.0, 0.0)];
const TRANSFORMED_PATH_POINTS: [(f32, f32); PATH_POINT_COUNT] = [(10.0, 5.0), (50.0, 5.0)];
const PLAIN_HALF_LENGTH: f32 = 10.0;
const TRANSFORMED_HALF_LENGTH: f32 = 20.0;
const FOLLOWING_TEXT_X: f32 = 40.0;
const FOLLOWING_TEXT_X_WITHOUT_PATH: f32 = 20.0;

fn parse_svg(input: &str) -> Result<usvg::Tree, String> {
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(input, &options).map_err(|error| error.to_string())
}

#[rstest]
#[case::valid("font-family='Yellowtail'", usvg::FontFamily::Named("Yellowtail".into()))]
#[case::invalid("font-family='Red/Black, sans-serif'", usvg::FontFamily::Named("Yellowtail".into()))]
#[case::inherited("", usvg::FontFamily::Serif)]
fn malformed_family_uses_configured_fallback_instead_of_parent(
    #[case] child_attribute: &str,
    #[case] expected: usvg::FontFamily,
) -> Result<(), String> {
    assert!(svgtypes::parse_font_families("Red/Black, sans-serif").is_err());
    let mut database = upstream_writer_font_database()?;
    std::sync::Arc::make_mut(&mut database).set_serif_family("Yellowtail");
    let options = usvg::Options {
        fontdb: database,
        font_family: "Yellowtail".to_owned(),
        ..usvg::Options::default()
    };
    let input = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><g font-family='serif'><text id='target' {child_attribute}>A</text></g></svg>"
    );
    let tree = usvg::Tree::from_str(&input, &options).map_err(|error| error.to_string())?;
    let span = text_node(&tree, "target")?
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .ok_or("text span missing")?;
    assert_eq!(span.font().families(), &[expected]);
    Ok(())
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
#[case::plain("", PLAIN_PATH_POINTS, PLAIN_HALF_LENGTH)]
#[case::transformed(
    "transform='matrix(2 0 0 2 10 5)'",
    TRANSFORMED_PATH_POINTS,
    TRANSFORMED_HALF_LENGTH
)]
fn text_path_uses_transformed_geometry_for_percentage_offset(
    #[case] transform: &str,
    #[case] expected_points: [(f32, f32); PATH_POINT_COUNT],
    #[case] expected_offset: f32,
) -> Result<(), String> {
    let input = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><defs><path id='curve' d='M0 0 L20 0' {transform}/></defs><text id='target' font-family='Yellowtail'><textPath href='#curve' startOffset='50%'>A</textPath></text></svg>"
    );
    let tree = parse_svg(&input)?;
    let chunk = text_node(&tree, "target")?
        .chunks()
        .first()
        .ok_or("chunk missing")?;
    let usvg::TextFlow::Path(flow) = chunk.text_flow() else {
        return Err("textPath flow missing".to_owned());
    };
    let points = flow.path().points();
    assert_eq!(points.len(), expected_points.len());
    for (point, (x, y)) in points.iter().zip(expected_points) {
        assert!((point.x - x).abs() < OFFSET_TOLERANCE);
        assert!((point.y - y).abs() < OFFSET_TOLERANCE);
    }
    assert!((flow.start_offset() - expected_offset).abs() < OFFSET_TOLERANCE);
    Ok(())
}

#[rstest]
#[case::missing("<textPath href='#missing'>BC</textPath>", "AD", "", FOLLOWING_TEXT_X)]
#[case::resolved(
    "<textPath href='#curve'>BC</textPath>",
    "ABCD",
    "BC",
    FOLLOWING_TEXT_X
)]
#[case::nested(
    "<tspan><textPath href='#curve'>BC</textPath></tspan>",
    "AD",
    "",
    FOLLOWING_TEXT_X_WITHOUT_PATH
)]
fn text_path_reference_preserves_surrounding_text_and_path_flow(
    #[case] path_text: &str,
    #[case] expected_text: &str,
    #[case] expected_path_text: &str,
    #[case] expected_following_text_x: f32,
) -> Result<(), String> {
    let input = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><defs><path id='curve' d='M0 20 L100 20'/></defs><text id='target' x='10 20 30 40' font-family='Yellowtail'>A{path_text}D</text></svg>"
    );
    let tree = parse_svg(&input)?;
    let text = text_node(&tree, "target")?;
    let chunks = text.chunks();
    let actual: String = chunks.iter().map(|chunk| chunk.text()).collect();
    assert_eq!(actual, expected_text);
    assert_eq!(text_in_flow(chunks, true), expected_path_text);
    assert_eq!(text_in_flow(chunks, false), "AD");
    assert_eq!(
        chunks.last().and_then(usvg::TextChunk::x),
        Some(expected_following_text_x)
    );
    Ok(())
}

fn text_in_flow(chunks: &[usvg::TextChunk], path: bool) -> String {
    chunks
        .iter()
        .filter(|chunk| matches!(chunk.text_flow(), usvg::TextFlow::Path(_)) == path)
        .map(|chunk| chunk.text())
        .collect()
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
    assert_target_text(&input, expected)
}

#[test]
fn tref_without_a_reference_preserves_adjacent_text() -> Result<(), String> {
    let input = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><text id='target' font-family='Yellowtail'>A<tref/>B</text></svg>";
    assert_target_text(input, "AB")
}

#[test]
fn tref_rejects_non_svg_namespace_target_and_keeps_adjacent_text() -> Result<(), String> {
    let input = "<svg xmlns='http://www.w3.org/2000/svg' xmlns:other='urn:other' width='100' height='50'><defs><other:text id='source'>X</other:text></defs><text id='target' font-family='Yellowtail'>A<tref href='#source'/>B</text></svg>";
    assert_target_text(input, "AB")
}

#[test]
fn default_whitespace_trims_both_edges_across_three_text_spans() -> Result<(), String> {
    let input = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><text id='target' font-family='Yellowtail'> A<tspan>B</tspan><tspan>C </tspan></text></svg>";
    assert_target_text(input, "ABC")
}

#[test]
fn whitespace_only_final_span_does_not_leave_trailing_space() -> Result<(), String> {
    let input = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><text id='target' font-family='Yellowtail'>A <tspan> </tspan></text></svg>";
    assert_target_text(input, "A")
}

fn assert_target_text(input: &str, expected: &str) -> Result<(), String> {
    let tree = parse_svg(input)?;
    let text = text_node(&tree, "target")?;
    let actual: String = text.chunks().iter().map(|chunk| chunk.text()).collect();
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn compressed_non_utf8_svg_reports_encoding_error() -> Result<(), String> {
    let gzip = [
        31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 251, 15, 0, 0, 0, 0, 255, 1, 0, 0, 0,
    ];
    let error = match usvg::Tree::from_data(&gzip, &usvg::Options::default()) {
        Ok(_) => return Err("non-UTF-8 gzip payload was accepted".to_owned()),
        Err(error) => error,
    };
    assert!(matches!(&error, usvg::Error::NotAnUtf8Str));
    assert_eq!(error.to_string(), "provided data has not an UTF-8 encoding");
    Ok(())
}

fn nested_fallback_resolver(
    styled: bool,
    observed: std::sync::Arc<std::sync::atomic::AtomicUsize>,
) -> usvg::FontResolver<'static> {
    use std::sync::atomic::Ordering;

    let mut resolver = usvg::FontResolver::default();
    if styled {
        resolver.select_styled_fallback = Some(Box::new(move |_, character, _, _| {
            if character == '\u{10fffd}' {
                observed.fetch_add(1, Ordering::Relaxed);
            }
            None
        }));
    } else {
        resolver.select_fallback = Box::new(move |character, _, _| {
            if character == '\u{10fffd}' {
                observed.fetch_add(1, Ordering::Relaxed);
            }
            None
        });
    }
    resolver
}

#[rstest]
#[case::legacy(false)]
#[case::styled(true)]
fn nested_svg_forwards_missing_glyph_fallback(#[case] styled: bool) -> Result<(), String> {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    let calls = Arc::new(AtomicUsize::new(0));
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        font_resolver: nested_fallback_resolver(styled, Arc::clone(&calls)),
        ..usvg::Options::default()
    };
    let input = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><text id='target' font-family='Yellowtail'>A\u{10fffd}</text></svg>";
    let tree = usvg::Tree::from_data_nested(input.as_bytes(), &options)
        .map_err(|error| error.to_string())?;
    assert!(tree.node_by_id("target").is_some());
    assert!(
        calls.load(Ordering::Relaxed) > 0,
        "nested fallback was not called"
    );
    Ok(())
}

#[rstest]
#[case::top_level(false, 1)]
#[case::nested(true, 0)]
fn nested_svg_disables_external_image_resolver(
    #[case] nested: bool,
    #[case] expected_calls: usize,
) -> Result<(), String> {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut options = usvg::Options::default();
    options.image_href_resolver.resolve_string = Box::new(move |_, _| {
        observed.fetch_add(1, Ordering::Relaxed);
        None
    });
    let input = b"<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><image id='external' href='other.svg' width='5' height='5'/><rect id='control' width='10' height='10'/></svg>";
    let result = if nested {
        usvg::Tree::from_data_nested(input, &options)
    } else {
        usvg::Tree::from_data(input, &options)
    };
    let tree = result.map_err(|error| error.to_string())?;
    assert_eq!(calls.load(Ordering::Relaxed), expected_calls);
    assert!(tree.node_by_id("external").is_none());
    assert!(tree.node_by_id("control").is_some());
    Ok(())
}
