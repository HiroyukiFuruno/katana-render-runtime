use super::{resvg, usvg};
use rstest::rstest;
use tiny_skia::{Pixmap, Transform};

#[path = "svg_engine_parser_linked_clip_tests.rs"]
mod linked_clip_tests;

const CLIP_VIEWPORT_WIDTH: u32 = 100;
const CLIP_VIEWPORT_HEIGHT: u32 = 80;
const OBJECT_BBOX_HALF_CLIP_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80">
  <defs><clipPath id="half" clipPathUnits="objectBoundingBox"><rect width="0.5" height="1"/></clipPath></defs>
  <rect x="10" y="10" width="40" height="20" fill="red" clip-path="url(#half)"/>
  <rect x="60" y="30" width="20" height="40" fill="blue" clip-path="url(#half)"/>
</svg>"#;
const OBJECT_BBOX_HALF_CLIP_ORACLE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80">
  <rect x="10" y="10" width="20" height="20" fill="red"/>
  <rect x="60" y="30" width="10" height="40" fill="blue"/>
</svg>"#;
const WRONG_CLIP_TARGET_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80">
  <defs><rect id="not-a-clip-path" width="4" height="4"/></defs>
  <rect x="10" y="10" width="30" height="20" fill="red" clip-path="url(#not-a-clip-path)"/>
</svg>"#;
const WRONG_CLIP_TARGET_ORACLE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80">
  <rect x="10" y="10" width="30" height="20" fill="red"/>
</svg>"#;

#[rstest]
#[case::markers_first("markers fill stroke", "red", "blue", &["marker", "both"][..])]
#[case::markers_last("fill stroke markers", "red", "blue", &["both", "marker"][..])]
#[case::fill_first("fill markers stroke", "red", "blue", &["fill", "marker", "stroke"][..])]
#[case::stroke_first("stroke markers fill", "red", "blue", &["stroke", "marker", "fill"][..])]
#[case::no_fill("fill markers stroke", "none", "blue", &["marker", "stroke"][..])]
#[case::no_stroke("stroke markers fill", "red", "none", &["marker", "fill"][..])]
#[case::repeated_marker("markers markers stroke", "red", "blue", &["both", "marker"][..])]
fn marker_paint_order_preserves_the_authored_sequence(
    #[case] order: &str,
    #[case] fill: &str,
    #[case] stroke: &str,
    #[case] expected: &[&str],
) -> Result<(), String> {
    let source = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40">
        <defs><marker id="m" markerWidth="4" markerHeight="4" orient="auto">
        <path d="M0 0 L4 2 L0 4 Z" fill="green"/></marker></defs>
        <path d="M5 5 L30 5 L30 30 Z" fill="{fill}" stroke="{stroke}"
        marker-start="url(#m)" paint-order="{order}"/></svg>"##
    );
    let tree = usvg::Tree::from_str(&source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let sequence = marker_sibling_sequence(tree.root()).ok_or("marker siblings missing")?;
    assert_eq!(sequence, expected);
    Ok(())
}

fn marker_sibling_sequence(group: &usvg::Group) -> Option<Vec<&'static str>> {
    let children = group.children();
    if children
        .iter()
        .any(|node| matches!(node, usvg::Node::Path(_)))
        && children
            .iter()
            .any(|node| matches!(node, usvg::Node::Group(_)))
    {
        return children.iter().map(classify_sibling).collect();
    }
    children.iter().find_map(|node| match node {
        usvg::Node::Group(child) => marker_sibling_sequence(child),
        _ => None,
    })
}

fn classify_sibling(node: &usvg::Node) -> Option<&'static str> {
    match node {
        usvg::Node::Group(_) => Some("marker"),
        usvg::Node::Path(path) => match (path.fill().is_some(), path.stroke().is_some()) {
            (true, true) => Some("both"),
            (true, false) => Some("fill"),
            (false, true) => Some("stroke"),
            (false, false) => None,
        },
        _ => None,
    }
}

#[test]
fn marker_first_stroke_then_fill_preserves_the_path_paint_order() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40">
      <defs><marker id="m" markerWidth="4" markerHeight="4"><path d="M0 0 L4 2 L0 4 Z"/></marker></defs>
      <path id="sample" d="M5 5 L30 5 L30 30 Z" fill="red" stroke="blue"
      marker-start="url(#m)" paint-order="markers stroke fill"/></svg>"##;
    let tree = usvg::Tree::from_str(source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let Some(usvg::Node::Path(path)) = tree.node_by_id("sample") else {
        return Err("sample path missing".to_owned());
    };
    assert_eq!(path.paint_order(), usvg::PaintOrder::StrokeAndFill);
    assert_eq!(
        marker_sibling_sequence(tree.root()).ok_or("marker siblings missing")?,
        ["marker", "both"]
    );
    Ok(())
}

#[rstest]
#[case::height_only(r#"height="25" viewBox="0 0 200 100""#, (50.0, 25.0))]
#[case::width_only(r#"width="50" viewBox="0 0 200 100""#, (50.0, 25.0))]
#[case::percentages(r#"width="50%" height="25%" viewBox="10 20 200 100""#, (100.0, 25.0))]
fn root_dimensions_resolve_against_viewbox(
    #[case] attributes: &str,
    #[case] expected: (f32, f32),
) -> Result<(), String> {
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" {attributes}><rect width="200" height="100"/></svg>"#
    );
    let tree = usvg::Tree::from_str(&source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    assert_eq!((tree.size().width(), tree.size().height()), expected);
    Ok(())
}

#[rstest]
#[case::zero_width("width=\"0\" height=\"10\"")]
#[case::zero_height("width=\"10\" height=\"0\"")]
fn root_rejects_nonpositive_explicit_dimensions(#[case] dimensions: &str) {
    let source = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" {dimensions}><rect width=\"10\" height=\"10\"/></svg>"
    );
    assert!(matches!(
        usvg::Tree::from_str(&source, &usvg::Options::default()),
        Err(usvg::Error::InvalidSize)
    ));
}

#[rstest]
#[case::solid_color("red", true)]
#[case::non_color_paint("url(#missing)", false)]
fn root_background_color_adds_only_a_solid_background_path(
    #[case] background: &str,
    #[case] expected: bool,
) -> Result<(), String> {
    let source = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\" background-color=\"{background}\"><rect width=\"10\" height=\"10\"/></svg>"
    );
    let tree = usvg::Tree::from_str(&source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    if expected {
        let [usvg::Node::Group(group)] = tree.root().children() else {
            return Err("solid root background did not create a wrapper group".to_owned());
        };
        let Some(usvg::Node::Path(path)) = group.children().first() else {
            return Err("solid root background path missing".to_owned());
        };
        assert!(matches!(
            path.fill().map(|fill| fill.paint()),
            Some(usvg::Paint::Color(color)) if (color.red, color.green, color.blue) == (255, 0, 0)
        ));
    } else {
        assert!(matches!(tree.root().children(), [usvg::Node::Path(_)]));
    }
    Ok(())
}

#[test]
fn percentage_root_without_viewbox_uses_the_default_viewport() -> Result<(), String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="50%" height="25%"></svg>"#;
    let tree = usvg::Tree::from_str(source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    assert_eq!(tree.size().width(), 50.0);
    assert_eq!(tree.size().height(), 25.0);
    assert!(tree.root().children().is_empty());
    Ok(())
}

#[test]
fn invalid_isolation_value_falls_back_without_dropping_the_shape() -> Result<(), String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><g style="isolation:invalid"><rect id="sample" width="10" height="10" fill="red"/></g></svg>"#;
    let tree = usvg::Tree::from_str(source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let Some(usvg::Node::Path(path)) = tree.node_by_id("sample") else {
        return Err("sample path missing".to_owned());
    };
    assert!(
        matches!(path.fill().map(|fill| fill.paint()), Some(usvg::Paint::Color(color)) if (color.red, color.green, color.blue) == (255, 0, 0))
    );
    Ok(())
}

#[test]
fn recursive_marker_keeps_one_marker_without_recursing() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40">
      <defs><marker id="m" markerWidth="4" markerHeight="4" overflow="visible">
        <path d="M0 0 L4 2 L0 4 Z" fill="green" marker-start="url(#m)"/>
      </marker></defs>
      <path id="sample" d="M5 5 L30 5" fill="none" stroke="blue" marker-start="url(#m)"/>
    </svg>"##;
    let tree = usvg::Tree::from_str(source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    assert_eq!(
        marker_sibling_sequence(tree.root()).ok_or("marker siblings missing")?,
        ["stroke", "marker"]
    );
    assert_eq!(marker_path_transforms(tree.root()).len(), 1);
    Ok(())
}

#[test]
fn object_bounding_box_clip_is_resolved_per_target_size() -> Result<(), String> {
    assert_same_visible_pixels(OBJECT_BBOX_HALF_CLIP_SVG, OBJECT_BBOX_HALF_CLIP_ORACLE_SVG)
}

#[test]
fn non_clippath_reference_leaves_the_target_unclipped() -> Result<(), String> {
    assert_same_visible_pixels(WRONG_CLIP_TARGET_SVG, WRONG_CLIP_TARGET_ORACLE_SVG)
}

fn assert_same_visible_pixels(input: &str, expected: &str) -> Result<(), String> {
    let rendered = render_svg(input)?;
    let oracle = render_svg(expected)?;
    assert!(
        rendered.pixels().iter().any(|pixel| pixel.alpha() != 0),
        "the input rendered no visible pixels"
    );
    assert_eq!(rendered.data(), oracle.data());
    Ok(())
}

fn render_svg(source: &str) -> Result<Pixmap, String> {
    let tree = usvg::Tree::from_str(source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let mut pixmap = Pixmap::new(CLIP_VIEWPORT_WIDTH, CLIP_VIEWPORT_HEIGHT)
        .ok_or("clip comparison pixmap unavailable")?;
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    Ok(pixmap)
}

#[test]
fn marker_before_new_subpath_uses_the_degenerate_cubic_incoming_direction() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40">
      <defs><marker id="m" markerUnits="userSpaceOnUse" markerWidth="4" markerHeight="4"
      orient="auto" overflow="visible"><path d="M0 0 L4 2 L0 4 Z" fill="green"/></marker></defs>
      <path id="sample" d="M5 5 C5 15 20 20 20 20 M25 20 L35 20"
      fill="none" stroke="blue" marker-mid="url(#m)"/>
    </svg>"##;
    let tree = usvg::Tree::from_str(source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let transforms = marker_path_transforms(tree.root());
    let [first, second] = transforms.as_slice() else {
        return Err(format!(
            "expected two middle markers, got {}",
            transforms.len()
        ));
    };
    let diagonal = std::f32::consts::FRAC_1_SQRT_2;
    assert!((first.sx - diagonal).abs() < 0.001);
    assert!((first.ky - diagonal).abs() < 0.001);
    assert!((first.tx - 20.0).abs() < 0.001 && (first.ty - 20.0).abs() < 0.001);
    assert!((second.sx - 1.0).abs() < 0.001 && second.ky.abs() < 0.001);
    Ok(())
}

fn marker_path_transforms(group: &usvg::Group) -> Vec<tiny_skia::Transform> {
    group
        .children()
        .iter()
        .flat_map(|node| match node {
            usvg::Node::Group(child) => marker_path_transforms(child),
            usvg::Node::Path(path) if path.id().is_empty() => vec![path.abs_transform()],
            _ => Vec::new(),
        })
        .collect()
}

#[rstest]
#[case::auto("auto", false)]
#[case::isolate("isolate", true)]
fn none_filter_preserves_group_isolation_and_shape(
    #[case] isolation: &str,
    #[case] expected: bool,
) -> Result<(), String> {
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><g id="holder" filter="none" style="isolation:{isolation}"><rect id="sample" width="10" height="10" fill="red"/></g></svg>"#
    );
    let tree = usvg::Tree::from_str(&source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let Some(usvg::Node::Group(group)) = tree.node_by_id("holder") else {
        return Err("holder group missing".to_owned());
    };
    assert_eq!(group.isolate(), expected);
    assert!(group.filters().is_empty());
    assert!(matches!(
        tree.node_by_id("sample"),
        Some(usvg::Node::Path(_))
    ));
    Ok(())
}
