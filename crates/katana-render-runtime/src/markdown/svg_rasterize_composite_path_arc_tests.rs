use super::{
    geometry, prepare_path, prepare_source_svg_arc, render_if_supported_with_tree,
    render_prepared_path,
};
use resvg::usvg;
use skia_safe::{Path, PathFillType, PathVerb};
use tiny_skia::{Pixmap, Transform};

const SVG_PREFIX: &str =
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><path d=""##;
const SVG_SUFFIX: &str = r##"" fill="#e02020" stroke="#2040e0" stroke-width="2"/></svg>"##;
const CANVAS_SIZE: u32 = 64;

#[test]
fn source_arc_matches_skia_svg_path_parser_for_relative_flags_and_closure() -> Result<(), String> {
    for data in [
        "M8 32 a16 12 0 0 1 32 0",
        "M8 32 A16 12 20 1 0 40 32",
        "M8 32 A16 12 0 0 0 40 32 Z",
    ] {
        assert_arc_matches_skia_parser(data)?;
    }
    assert_even_odd_arc_fill_type()?;
    Ok(())
}

fn assert_arc_matches_skia_parser(data: &str) -> Result<(), String> {
    let tree = parse_path_fixture(data)?;
    let node = first_path(&tree)?;
    let source = tree
        .source_svg_path(node)
        .ok_or("source SVG arc metadata missing")?;
    let expected = Path::from_svg(source).ok_or("Skia SVG path parser rejected source")?;
    let fill_type = node_fill_type(node);
    let actual =
        geometry::source_svg_arc(source, fill_type).ok_or("native source arc was not accepted")?;
    assert!(actual.verbs().contains(&PathVerb::Conic));
    assert_eq!(actual.verbs(), expected.verbs());
    assert_eq!(actual.points(), expected.points());
    assert_eq!(actual.conic_weights(), expected.conic_weights());
    assert_eq!(actual.fill_type(), fill_type);
    Ok(())
}

fn node_fill_type(node: &usvg::Path) -> PathFillType {
    node.fill()
        .map(|fill| match fill.rule() {
            usvg::FillRule::NonZero => PathFillType::Winding,
            usvg::FillRule::EvenOdd => PathFillType::EvenOdd,
        })
        .unwrap_or(PathFillType::Winding)
}

fn assert_even_odd_arc_fill_type() -> Result<(), String> {
    let even_odd = parse_svg(
        &format!("{SVG_PREFIX}M8 32 A16 12 0 0 1 40 32 Z{SVG_SUFFIX}")
            .replace("<path d=", "<path fill-rule=\"evenodd\" d="),
    )?;
    let node = first_path(&even_odd)?;
    let source = even_odd
        .source_svg_path(node)
        .ok_or("even-odd source SVG arc metadata missing")?;
    let path = geometry::source_svg_arc(source, PathFillType::EvenOdd)
        .ok_or("even-odd source SVG arc was not accepted")?;
    assert_eq!(path.fill_type(), PathFillType::EvenOdd);
    Ok(())
}

#[test]
fn source_arc_keeps_prepared_paint_order_and_matches_parser_path() -> Result<(), String> {
    let fill_then_stroke = render_arc_fixture("fill stroke")?;
    let stroke_then_fill = render_arc_fixture("stroke fill")?;
    assert_eq!(fill_then_stroke.0.data(), fill_then_stroke.1.data());
    assert_eq!(stroke_then_fill.0.data(), stroke_then_fill.1.data());
    assert_ne!(fill_then_stroke.0.data(), stroke_then_fill.0.data());
    Ok(())
}

#[test]
fn source_arc_falls_back_for_out_of_bounds_and_dash_effects() -> Result<(), String> {
    let tree = parse_path_fixture("M8 32 A16 12 0 0 1 40 32")?;
    assert_arc_falls_back(&tree, Transform::from_translate(-(CANVAS_SIZE as f32), 0.0))?;

    let dashed = parse_svg(&dashed_arc_fixture())?;
    assert_arc_falls_back(&dashed, Transform::identity())?;
    Ok(())
}

fn dashed_arc_fixture() -> String {
    format!("{SVG_PREFIX}M8 32 A16 12 0 0 1 40 32{SVG_SUFFIX}").replace(
        "stroke-width=\"2\"",
        "stroke-width=\"2\" stroke-dasharray=\"4 2\"",
    )
}

fn assert_arc_falls_back(tree: &usvg::Tree, transform: Transform) -> Result<(), String> {
    let node = first_path(tree)?;
    let prepared = prepare_path(node)?.ok_or("arc fixture paints were not prepared")?;
    let mut target = canvas()?;
    let prepared = prepare_source_svg_arc(node, Some(tree), transform, &target.as_mut(), prepared);
    assert!(!prepared.path.verbs().contains(&PathVerb::Conic));
    Ok(())
}

#[test]
fn source_arc_requires_valid_conic_and_keeps_ordinary_cubic_unchanged() -> Result<(), String> {
    assert!(geometry::source_svg_arc("M0 0 L10 10", PathFillType::Winding).is_none());
    assert!(geometry::source_svg_arc("M0 0 A bad", PathFillType::Winding).is_none());

    let tree = parse_path_fixture("M8 8 C16 8 24 24 32 32")?;
    let node = first_path(&tree)?;
    assert!(tree.source_svg_path(node).is_none());
    let mut with_tree = canvas()?;
    let mut without_tree = canvas()?;
    render_if_supported_with_tree(
        node,
        Transform::identity(),
        &mut with_tree.as_mut(),
        Some(&tree),
    )?;
    render_if_supported_with_tree(
        node,
        Transform::identity(),
        &mut without_tree.as_mut(),
        None,
    )?;
    assert_eq!(with_tree.data(), without_tree.data());
    Ok(())
}

fn render_arc_fixture(order: &str) -> Result<(Pixmap, Pixmap), String> {
    let tree = parse_svg(&arc_fixture(order))?;
    let node = first_path(&tree)?;
    Ok((
        render_actual_arc(&tree, node)?,
        render_parser_arc(&tree, node)?,
    ))
}

fn arc_fixture(order: &str) -> String {
    format!("{SVG_PREFIX}M8 32 A16 12 0 0 1 40 32 L40 48 L8 48 Z{SVG_SUFFIX}").replace(
        "stroke-width=\"2\"",
        &format!("stroke-width=\"2\" paint-order=\"{order}\""),
    )
}

fn render_actual_arc(tree: &usvg::Tree, node: &usvg::Path) -> Result<Pixmap, String> {
    let mut actual = canvas()?;
    if !render_if_supported_with_tree(
        node,
        Transform::identity(),
        &mut actual.as_mut(),
        Some(tree),
    )? {
        return Err("source arc did not use the supported Skia path route".to_owned());
    }
    Ok(actual)
}

fn render_parser_arc(tree: &usvg::Tree, node: &usvg::Path) -> Result<Pixmap, String> {
    let source = tree
        .source_svg_path(node)
        .ok_or("source SVG arc metadata missing")?;
    let mut prepared = prepare_path(node)?.ok_or("arc paints were not prepared")?;
    prepared.path = geometry::source_svg_arc(source, prepared.path.fill_type())
        .ok_or("Skia SVG path parser rejected source")?;
    let mut expected = canvas()?;
    render_prepared_path(
        &mut expected.as_mut(),
        &prepared,
        Transform::identity(),
        node.paint_order(),
    )?;
    Ok(expected)
}

fn parse_path_fixture(data: &str) -> Result<usvg::Tree, String> {
    parse_svg(&format!("{SVG_PREFIX}{data}{SVG_SUFFIX}"))
}

fn parse_svg(svg: &str) -> Result<usvg::Tree, String> {
    usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|error| error.to_string())
}

fn first_path(tree: &usvg::Tree) -> Result<&usvg::Path, String> {
    tree.root()
        .children()
        .iter()
        .find_map(|node| match node {
            usvg::Node::Path(path) => Some(path.as_ref()),
            _ => None,
        })
        .ok_or_else(|| "arc fixture produced no root path".to_owned())
}

fn canvas() -> Result<Pixmap, String> {
    Pixmap::new(CANVAS_SIZE, CANVAS_SIZE)
        .ok_or_else(|| "failed to allocate arc fixture pixmap".to_owned())
}
