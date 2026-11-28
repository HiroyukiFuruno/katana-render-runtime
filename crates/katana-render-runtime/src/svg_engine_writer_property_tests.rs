use std::str::FromStr;

use super::{parse_writer_svg, render_tree, usvg, writer_options};

const SHAPE_RENDERING_CASES: [(&str, &str, usvg::ShapeRendering, bool); 3] = [
    (
        "speed",
        "optimizeSpeed",
        usvg::ShapeRendering::OptimizeSpeed,
        false,
    ),
    (
        "crisp",
        "crispEdges",
        usvg::ShapeRendering::CrispEdges,
        false,
    ),
    (
        "precision",
        "geometricPrecision",
        usvg::ShapeRendering::GeometricPrecision,
        true,
    ),
];

const SHAPE_RENDERING_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32">
  <path id="speed" d="M 2 2 L 28 2 L 28 28 Z" fill="#d02020" shape-rendering="optimizeSpeed"/>
  <path id="crisp" d="M 4 4 L 24 4 L 24 24 Z" fill="#20a040" shape-rendering="crispEdges"/>
  <path id="precision" d="M 6 6 L 20 6 L 20 20 Z" fill="#2040d0" shape-rendering="geometricPrecision"/>
</svg>"##;

#[test]
fn shape_rendering_from_str_matches_antialias_policy() -> Result<(), String> {
    for (_, name, expected, use_antialiasing) in SHAPE_RENDERING_CASES {
        let parsed = usvg::ShapeRendering::from_str(name)
            .map_err(|error| format!("failed to parse shape-rendering {name}: {error}"))?;
        assert_eq!(parsed, expected, "parsed shape-rendering {name}");
        assert_eq!(parsed.use_shape_antialiasing(), use_antialiasing, "{name}");
    }
    assert!(usvg::ShapeRendering::from_str("unsupported").is_err());
    Ok(())
}

#[test]
fn writer_roundtrip_preserves_non_default_shape_rendering_modes() -> Result<(), String> {
    let original = parse_writer_svg(SHAPE_RENDERING_SVG, "shape-rendering-property")?;
    assert_tree_shape_rendering_modes(&original)?;
    let options = writer_options("shape-rendering-property");
    let serialized = original.to_string(&options);
    assert!(serialized.contains("shape-rendering=\"optimizeSpeed\""));
    assert!(serialized.contains("shape-rendering=\"crispEdges\""));
    let reparsed = parse_writer_svg(&serialized, "shape-rendering-property-roundtrip")?;
    assert_tree_shape_rendering_modes(&reparsed)?;
    assert_eq!(
        render_tree(&original)?.data(),
        render_tree(&reparsed)?.data()
    );
    Ok(())
}

fn assert_tree_shape_rendering_modes(tree: &usvg::Tree) -> Result<(), String> {
    for (id, _, expected, _) in SHAPE_RENDERING_CASES {
        assert_node_shape_rendering(tree, id, expected)?;
    }
    Ok(())
}

fn assert_node_shape_rendering(
    tree: &usvg::Tree,
    id: &str,
    expected: usvg::ShapeRendering,
) -> Result<(), String> {
    let node = tree
        .node_by_id(id)
        .ok_or_else(|| format!("missing path {id}"))?;
    let usvg::Node::Path(path) = node else {
        return Err(format!("node {id} is not a path"));
    };
    assert_eq!(path.rendering_mode(), expected, "rendering mode for {id}");
    assert_eq!(
        path.rendering_mode().use_shape_antialiasing(),
        expected.use_shape_antialiasing(),
        "antialias policy for {id}"
    );
    Ok(())
}
