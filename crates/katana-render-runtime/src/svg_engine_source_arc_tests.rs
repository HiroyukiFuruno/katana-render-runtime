use super::usvg;
use std::sync::Arc;

fn parse(svg: &str) -> Result<usvg::Tree, String> {
    usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|error| error.to_string())
}

fn collect_paths<'a>(group: &'a usvg::Group, paths: &mut Vec<&'a usvg::Path>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_paths(group, paths),
            usvg::Node::Path(path) => paths.push(path),
            _ => {}
        }
    }
}

#[test]
fn arc_source_preserves_idless_absolute_and_relative_commands() -> Result<(), String> {
    let absolute = "M1 2 A5 6 30 1 0 20 22";
    let relative = "M2 3 a5 6 30 0 1 20 22";
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80"><path d="{absolute}"/><path d="{relative}"/><path d="M1 2 C3 4 5 6 7 8"/></svg>"#
    );
    let tree = parse(&svg)?;
    let mut paths = Vec::new();
    collect_paths(tree.root(), &mut paths);
    assert_eq!(paths.len(), 3);
    assert_eq!(tree.source_svg_path(paths[0]), Some(absolute));
    assert_eq!(tree.source_svg_path(paths[1]), Some(relative));
    assert_eq!(tree.source_svg_path(paths[2]), None);
    Ok(())
}

#[test]
fn arc_source_survives_clone_and_use_but_not_new_geometry_or_writer() -> Result<(), String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80"><defs><path id="p" d="M1 2 a5 6 30 0 1 20 22"/></defs><use href="#p" x="4"/></svg>"##;
    let tree = parse(svg)?;
    let other = parse(svg)?;
    let mut paths = Vec::new();
    collect_paths(tree.root(), &mut paths);
    let first = paths.first().ok_or("use path missing")?;
    let mut cloned = (*first).clone();
    assert!(tree.source_svg_path(&cloned).is_some());
    assert_eq!(other.source_svg_path(&cloned), None);
    cloned.data = Arc::new(
        cloned
            .data()
            .clone()
            .transform(usvg::Transform::from_translate(1.0, 0.0))
            .ok_or("transform failed")?,
    );
    assert_eq!(tree.source_svg_path(&cloned), None);
    let written = tree.to_string(&usvg::WriteOptions::default());
    let reparsed = parse(&written)?;
    let mut written_paths = Vec::new();
    collect_paths(reparsed.root(), &mut written_paths);
    assert!(
        written_paths
            .iter()
            .all(|path| reparsed.source_svg_path(path).is_none())
    );
    Ok(())
}

#[test]
fn arc_source_rejects_invalid_suffix_and_nonfinite_values() -> Result<(), String> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80"><path d="M0 0 A5 5 0 0 1 10 0 L"/><path d="M0 0 A5 5 0 0 1 10 0 L1e999 2"/><path d="M0 0 A-5 5 0 0 1 10 0"/></svg>"#;
    let tree = parse(svg)?;
    let mut paths = Vec::new();
    collect_paths(tree.root(), &mut paths);
    assert!(
        !paths.is_empty(),
        "valid prefix should retain its legacy geometry"
    );
    assert!(
        paths
            .iter()
            .all(|path| tree.source_svg_path(path).is_none())
    );
    Ok(())
}

#[test]
fn nested_svg_arc_source_is_scoped_to_its_tree() -> Result<(), String> {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80"><path d="M0 0 A5 5 0 0 1 10 0"/><image width="20" height="20" href="data:image/svg+xml,%3Csvg%20xmlns%3D%27http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%27%20width%3D%2720%27%20height%3D%2720%27%3E%3Cpath%20d%3D%27M0%200%20a5%205%200%200%201%2010%200%27%2F%3E%3C%2Fsvg%3E"/></svg>"#;
    let outer = parse(input)?;
    let inner =
        super::source_ellipse_tests::nested_svg(outer.root()).ok_or("nested SVG missing")?;
    let mut outer_paths = Vec::new();
    let mut inner_paths = Vec::new();
    collect_paths(outer.root(), &mut outer_paths);
    collect_paths(inner.root(), &mut inner_paths);
    let outer_path = *outer_paths.first().ok_or("outer path missing")?;
    let inner_path = *inner_paths.first().ok_or("inner path missing")?;
    assert!(outer.source_svg_path(outer_path).is_some());
    assert!(inner.source_svg_path(inner_path).is_some());
    assert_eq!(outer.source_svg_path(inner_path), None);
    assert_eq!(inner.source_svg_path(outer_path), None);
    Ok(())
}
