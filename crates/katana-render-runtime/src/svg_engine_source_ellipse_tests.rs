use super::usvg;
use std::sync::Arc;

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80">
  <circle id="circle" cx="20" cy="30" r="10"/>
  <ellipse id="ellipse" cx="50%" cy="25%" rx="12" ry="8"/>
  <rect id="rect" width="10" height="10"/>
  <path id="path" d="M0 0L10 10"/>
</svg>"##;

fn parse(svg: &str) -> Result<usvg::Tree, String> {
    usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|error| error.to_string())
}

fn path<'a>(tree: &'a usvg::Tree, id: &str) -> Result<&'a usvg::Path, String> {
    match tree.node_by_id(id) {
        Some(usvg::Node::Path(path)) => Ok(path),
        _ => Err(format!("path {id} missing")),
    }
}

#[test]
fn source_ellipse_records_resolved_local_values_only() -> Result<(), String> {
    let tree = parse(SVG)?;
    assert_eq!(
        tree.source_ellipse(path(&tree, "circle")?),
        Some((20.0, 30.0, 10.0, 10.0))
    );
    assert_eq!(
        tree.source_ellipse(path(&tree, "ellipse")?),
        Some((50.0, 20.0, 12.0, 8.0))
    );
    assert_eq!(tree.source_ellipse(path(&tree, "rect")?), None);
    assert_eq!(tree.source_ellipse(path(&tree, "path")?), None);
    Ok(())
}

#[test]
fn source_ellipse_shares_clones_but_not_replaced_geometry_or_other_trees() -> Result<(), String> {
    let tree = parse(SVG)?;
    let other = parse(SVG)?;
    let tree_clone = tree.clone();
    assert!(Arc::ptr_eq(
        tree.source_shapes.as_ref().ok_or("registry missing")?,
        tree_clone
            .source_shapes
            .as_ref()
            .ok_or("clone registry missing")?,
    ));
    let mut cloned = path(&tree, "circle")?.clone();
    assert_eq!(tree.source_ellipse(&cloned), Some((20.0, 30.0, 10.0, 10.0)));
    assert_eq!(other.source_ellipse(&cloned), None);
    let transformed = cloned
        .data()
        .clone()
        .transform(usvg::Transform::from_translate(1.0, 0.0))
        .ok_or("transform failed")?;
    cloned.data = Arc::new(transformed);
    assert_eq!(tree.source_ellipse(&cloned), None);
    Ok(())
}

#[test]
fn source_ellipse_handles_use_and_writer_roundtrip() -> Result<(), String> {
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80">
      <defs><circle id="source" cx="20" cy="30" r="10"/></defs>
      <use href="#source" x="5"/></svg>"##;
    let tree = parse(input)?;
    let mut values = Vec::new();
    collect_ellipses(&tree, tree.root(), &mut values);
    assert_eq!(values, vec![(20.0, 30.0, 10.0, 10.0)]);
    let written = tree.to_string(&usvg::WriteOptions::default());
    let reparsed = parse(&written)?;
    values.clear();
    collect_ellipses(&reparsed, reparsed.root(), &mut values);
    assert!(values.is_empty());
    Ok(())
}

#[test]
fn source_ellipse_follows_existing_invalid_radius_resolution() -> Result<(), String> {
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80">
      <circle id="zero" r="0"/><ellipse id="negative" rx="-1" ry="5"/>
      <ellipse id="reference" ry="5"/></svg>"##;
    let tree = parse(input)?;
    assert!(tree.node_by_id("zero").is_none());
    let negative = path(&tree, "negative")?;
    let reference = path(&tree, "reference")?;
    assert_eq!(tree.source_ellipse(negative), Some((0.0, 0.0, 5.0, 5.0)));
    assert_eq!(
        tree.source_ellipse(negative),
        tree.source_ellipse(reference)
    );
    assert!(negative.data() == reference.data());
    Ok(())
}

fn collect_ellipses(
    tree: &usvg::Tree,
    group: &usvg::Group,
    values: &mut Vec<(f32, f32, f32, f32)>,
) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_ellipses(tree, group, values),
            usvg::Node::Path(path) => values.extend(tree.source_ellipse(path)),
            _ => {}
        }
    }
}

#[test]
fn nested_svg_image_keeps_registry_separate() -> Result<(), String> {
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80">
      <circle id="outer" cx="20" cy="30" r="10"/>
      <image width="20" height="20" href="data:image/svg+xml,%3Csvg%20xmlns%3D%27http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%27%20width%3D%2720%27%20height%3D%2720%27%3E%3Ccircle%20id%3D%27inner%27%20cx%3D%275%27%20cy%3D%275%27%20r%3D%273%27%2F%3E%3C%2Fsvg%3E"/>
    </svg>"##;
    let outer = parse(input)?;
    let inner = nested_svg(outer.root()).ok_or("nested SVG missing")?;
    let outer_path = path(&outer, "outer")?;
    let inner_path = path(inner, "inner")?;
    assert!(outer.source_ellipse(outer_path).is_some());
    assert!(inner.source_ellipse(inner_path).is_some());
    assert_eq!(outer.source_ellipse(inner_path), None);
    assert_eq!(inner.source_ellipse(outer_path), None);
    Ok(())
}

pub(super) fn nested_svg(group: &usvg::Group) -> Option<&usvg::Tree> {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => {
                if let Some(tree) = nested_svg(group) {
                    return Some(tree);
                }
            }
            usvg::Node::Image(image) => {
                if let usvg::ImageKind::SVG(tree) = image.kind() {
                    return Some(tree);
                }
            }
            _ => {}
        }
    }
    None
}
