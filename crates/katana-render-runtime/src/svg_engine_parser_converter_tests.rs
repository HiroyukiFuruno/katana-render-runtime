use super::usvg;
use rstest::rstest;

#[rstest]
#[case::markers_first("markers fill stroke", "red", "blue", &["marker", "both"][..])]
#[case::markers_last("fill stroke markers", "red", "blue", &["both", "marker"][..])]
#[case::fill_first("fill markers stroke", "red", "blue", &["fill", "marker", "stroke"][..])]
#[case::stroke_first("stroke markers fill", "red", "blue", &["stroke", "marker", "fill"][..])]
#[case::no_fill("fill markers stroke", "none", "blue", &["marker", "stroke"][..])]
#[case::no_stroke("stroke markers fill", "red", "none", &["marker", "fill"][..])]
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
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><g isolation="invalid"><rect id="sample" width="10" height="10" fill="red"/></g></svg>"#;
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
