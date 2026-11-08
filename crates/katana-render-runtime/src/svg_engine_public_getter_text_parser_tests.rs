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
