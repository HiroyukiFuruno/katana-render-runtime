use super::{Mode, render, upstream_writer_font_database, usvg};
use rstest::rstest;
use std::path::PathBuf;
use tiny_skia::Pixmap;

#[path = "svg_engine_render_marker_geometry_tests.rs"]
mod geometry;

#[path = "svg_engine_render_marker_recursion_tests.rs"]
mod recursion;

const FIXTURE_DIR: &str =
    "../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/painting/marker";
const SCALE: f32 = 1.0;
const LEFT_END: u32 = 60;
const RIGHT_START: u32 = 140;
const START_SAMPLE_X: u32 = 100;
const START_SAMPLE_Y: u32 = 10;
const CSS_MARKER_COUNT: usize = 5;
const CSS_MARKER_SCALE: f32 = 1.5;
const RGBA_CHANNELS: usize = 4;
const CSS_MARKER_BOUNDARY_PIXELS: [(u32, u32); CSS_MARKER_COUNT] =
    [(141, 37), (216, 277), (21, 127), (261, 127), (66, 277)];
const CSS_MARKER_BOUNDARY_RGBA: [u8; RGBA_CHANNELS] = [0, 0, 96, 96];

#[rstest]
#[case("only-marker-start", 1)]
#[case("only-marker-mid", 3)]
#[case("only-marker-end", 1)]
#[case("zero-sized", 0)]
#[case("zero-sized-stroke", 0)]
#[case("marker-with-a-negative-size", 0)]
#[case("recursive-1", 5)]
#[case("with-viewBox-1", 1)]
fn marker_expansion_respects_position_and_invalid_size(
    #[case] name: &str,
    #[case] expected: usize,
) -> Result<(), String> {
    let tree = parse_fixture(name)?;
    assert_eq!(marker_path_count(tree.root()), expected, "{name}");
    let image = render(&tree, Mode::Extra(SCALE))?;
    assert!(image.pixels().iter().any(|pixel| pixel.alpha() > 0));
    Ok(())
}

fn marker_path_count(group: &usvg::Group) -> usize {
    group
        .children()
        .iter()
        .map(|node| match node {
            usvg::Node::Group(group) => marker_path_count(group),
            usvg::Node::Path(path) if path.id().is_empty() => 1,
            _ => 0,
        })
        .sum()
}

#[test]
fn css_marker_shorthand_matches_explicit_start_mid_and_end() -> Result<(), String> {
    let source = fixture("the-marker-property-in-CSS")?;
    let explicit = source.replace(
        "marker:url(#marker1)",
        "marker-start:url(#marker1);marker-mid:url(#marker1);marker-end:url(#marker1)",
    );
    assert_ne!(source, explicit);
    let shorthand_tree = parse(&source)?;
    let explicit_tree = parse(&explicit)?;
    assert_eq!(marker_path_count(shorthand_tree.root()), CSS_MARKER_COUNT);
    assert_eq!(marker_path_count(explicit_tree.root()), CSS_MARKER_COUNT);
    let shorthand = render(&shorthand_tree, Mode::Extra(SCALE))?;
    let longhand = render(&explicit_tree, Mode::Extra(SCALE))?;
    assert!(
        shorthand.data() == longhand.data(),
        "marker shorthand pixels differ"
    );
    assert!(blue_pixels(&shorthand) > 0);
    Ok(())
}

#[test]
fn css_marker_lower_boundaries_preserve_single_antialias_coverage() -> Result<(), String> {
    let tree = parse_fixture("the-marker-property-in-CSS")?;
    let image = render(&tree, Mode::Extra(CSS_MARKER_SCALE))?;
    /* WHY: 不透明度0.75と半被覆0.5でalpha96となり、clipで二重に半被覆しない。 */
    for point in CSS_MARKER_BOUNDARY_PIXELS {
        assert_premultiplied_pixel(&image, point, CSS_MARKER_BOUNDARY_RGBA)?;
    }
    Ok(())
}

fn assert_premultiplied_pixel(
    image: &Pixmap,
    (x, y): (u32, u32),
    expected: [u8; RGBA_CHANNELS],
) -> Result<(), String> {
    let pixel = image.pixel(x, y).ok_or("marker boundary pixel missing")?;
    assert_eq!(
        [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()],
        expected,
        "marker boundary ({x}, {y})"
    );
    Ok(())
}

#[test]
fn invalid_marker_units_fall_back_to_stroke_width() -> Result<(), String> {
    let source = fixture("with-invalid-markerUnits")?;
    let explicit = source.replace("markerUnits=\"qwe\"", "markerUnits=\"strokeWidth\"");
    assert_ne!(source, explicit);
    let actual = render_source(&source)?;
    let expected = render_source(&explicit)?;
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

#[test]
fn user_space_marker_does_not_scale_with_stroke_width() -> Result<(), String> {
    let source = fixture("with-markerUnits=userSpaceOnUse")?;
    let stroke_scaled = source.replace(
        "markerUnits=\"userSpaceOnUse\"",
        "markerUnits=\"strokeWidth\"",
    );
    assert_ne!(source, stroke_scaled);
    let user = render_source(&source)?;
    let stroke = render_source(&stroke_scaled)?;
    assert!(blue_pixels(&stroke) > blue_pixels(&user));
    assert!(blue_pixels(&user) > 0);
    Ok(())
}

fn blue_pixels(image: &Pixmap) -> usize {
    image
        .pixels()
        .iter()
        .filter(|pixel| pixel.blue() > pixel.red() && pixel.blue() > pixel.green())
        .count()
}

#[test]
fn auto_start_reverse_changes_start_and_preserves_end() -> Result<(), String> {
    let source = fixture("orient=auto-start-reverse")?;
    let normal = source.replace("orient=\"auto-start-reverse\"", "orient=\"auto\"");
    assert_ne!(source, normal);
    let reverse = render_source(&source)?;
    let forward = render_source(&normal)?;
    assert!(regions_differ(&reverse, &forward, 0, LEFT_END));
    assert!(!regions_differ(
        &reverse,
        &forward,
        RIGHT_START,
        reverse.width()
    ));
    Ok(())
}

fn regions_differ(left: &Pixmap, right: &Pixmap, start: u32, end: u32) -> bool {
    (0..left.height()).any(|y| (start..end).any(|x| left.pixel(x, y) != right.pixel(x, y)))
}

#[test]
fn start_marker_renders_at_its_first_vertex() -> Result<(), String> {
    let source = fixture("only-marker-start")?;
    let no_marker = source.replace("marker-start=\"url(#marker1)\"", "");
    assert_ne!(source, no_marker);
    let with = render_source(&source)?;
    let without = render_source(&no_marker)?;
    let marker = with
        .pixel(START_SAMPLE_X, START_SAMPLE_Y)
        .ok_or("marker sample missing")?;
    let bare = without
        .pixel(START_SAMPLE_X, START_SAMPLE_Y)
        .ok_or("bare sample missing")?;
    assert!(marker.blue() > marker.red() && marker.blue() > marker.green());
    assert_eq!(bare.alpha(), 0);
    Ok(())
}

#[test]
fn marker_default_overflow_matches_hidden_and_clips_oversized_children() -> Result<(), String> {
    let source = fixture("default-clip")?;
    let hidden = source.replace(
        "markerHeight=\"20\">",
        "markerHeight=\"20\" overflow=\"hidden\">",
    );
    let visible = source.replace(
        "markerHeight=\"20\">",
        "markerHeight=\"20\" overflow=\"visible\">",
    );
    assert_ne!(source, hidden);
    assert_ne!(source, visible);
    let default_image = render_source(&source)?;
    let hidden_image = render_source(&hidden)?;
    let visible_image = render_source(&visible)?;
    assert_eq!(default_image.data(), hidden_image.data());
    assert!(blue_pixels(&visible_image) > blue_pixels(&default_image));
    Ok(())
}

#[test]
fn empty_marker_definition_adds_no_rendered_content() -> Result<(), String> {
    let source = fixture("empty")?;
    let without_marker = source.replace(
        "<marker id=\"marker1\" refX=\"16\" refY=\"16\" markerWidth=\"32\" markerHeight=\"32\"/>",
        "",
    );
    assert_ne!(source, without_marker);
    let with_marker = render_source(&source)?;
    let bare_path = render_source(&without_marker)?;
    assert_eq!(with_marker.data(), bare_path.data());
    Ok(())
}

#[test]
fn marker_on_shape_inside_clip_path_does_not_change_the_clip() -> Result<(), String> {
    const SOURCE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="24">
      <defs><marker id="m" markerWidth="12" markerHeight="12"><path d="M0 0L12 6L0 12Z" fill="red"/></marker>
      <clipPath id="c"><path d="M4 4H36V20H4Z" marker-start="url(#m)"/></clipPath></defs>
      <rect width="40" height="24" fill="blue" clip-path="url(#c)"/></svg>"##;
    let without_marker = SOURCE.replace(" marker-start=\"url(#m)\"", "");
    assert_ne!(SOURCE, without_marker);
    let marked = render_source(SOURCE)?;
    let plain_clip = render_source(&without_marker)?;
    assert_eq!(marked.data(), plain_clip.data());
    assert!(blue_pixels(&marked) > 0);
    Ok(())
}

fn render_source(source: &str) -> Result<Pixmap, String> {
    let tree = parse(source)?;
    render(&tree, Mode::Extra(SCALE))
}

fn parse_fixture(name: &str) -> Result<usvg::Tree, String> {
    parse(&fixture(name)?)
}

fn parse(source: &str) -> Result<usvg::Tree, String> {
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(source, &options).map_err(|error| error.to_string())
}

fn fixture(name: &str) -> Result<String, String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_DIR);
    std::fs::read_to_string(dir.join(format!("{name}.svg"))).map_err(|error| error.to_string())
}
