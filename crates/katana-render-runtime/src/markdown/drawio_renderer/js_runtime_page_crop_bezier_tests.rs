use super::super::DrawioJsRuntimeOps;
use super::super::page_crop_test_support::temp_runtime_path;
use crate::markdown::color_preset::DiagramColorPreset;
use resvg::usvg;
use std::sync::atomic::{AtomicUsize, Ordering};

const PATH_BOX_COORDINATES: usize = 4;
static BUNDLE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
const PATH_BOX_BUNDLE_PREFIX: &str = r#"
function Graph() {}
const Editor = { convertHtmlToText(value) { return String(value); } };
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "1200px");
  svg.setAttribute("height", "900px");
  svg.setAttribute("viewBox", "0 0 1200 900");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", "#;

#[test]
fn drawio_runtime_get_bbox_excludes_quadratic_control_points() -> Result<(), String> {
    let path = "M339 458.55 Q692 820 1105.78 499.09";
    let actual = drawio_get_bbox(path)?;

    assert_box_close(actual, [339.0, 458.55, 1105.78, 650.0121350899818])?;
    assert_path_box_matches_usvg(path, actual)
}

#[test]
fn drawio_runtime_get_bbox_uses_cubic_curve_extrema() -> Result<(), String> {
    let path = "M0 0 C0 100 100 100 100 0";
    let actual = drawio_get_bbox(path)?;

    assert_box_close(actual, [0.0, 0.0, 100.0, 75.0])?;
    assert_path_box_matches_usvg(path, actual)
}

#[test]
fn drawio_runtime_get_bbox_handles_relative_repeated_and_reflected_curves() -> Result<(), String> {
    for path in [
        "M10 10 q10 30 20 0 10 -30 20 0",
        "M0 0 c0 100 100 100 100 0 s100 -100 100 0",
        "M0 0 Q50 100 100 0 T200 0 M300 0 Q350 -100 400 0 T500 0",
        "M0 0 C0 100 100 100 100 0 S200 -100 200 0 M300 0 C350 50 400 50 400 0 S450 -50 500 0",
        "M0 0 10 10 Z Q30 20 40 0 T60 0",
    ] {
        let actual = drawio_get_bbox(path)?;
        assert_path_box_matches_usvg(path, actual)?;
    }
    Ok(())
}

#[test]
fn drawio_runtime_get_bbox_keeps_degenerate_and_subpath_endpoints() -> Result<(), String> {
    for (path, expected) in [
        ("M4 7", [4.0, 7.0, 4.0, 7.0]),
        ("M4 7 Q4 7 4 7", [4.0, 7.0, 4.0, 7.0]),
        ("M0 5 C3 5 7 5 10 5", [0.0, 5.0, 10.0, 5.0]),
        ("M4 7 L4 7 M10 20 L10 20", [4.0, 7.0, 10.0, 20.0]),
    ] {
        assert_box_close(drawio_get_bbox(path)?, expected)?;
    }
    Ok(())
}

fn drawio_get_bbox(path_data: &str) -> Result<[f64; PATH_BOX_COORDINATES], String> {
    let bundle = path_box_bundle(path_data)?;
    let sequence = BUNDLE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let bundle_path = temp_runtime_path(&format!("krr-drawio-bezier-runtime-unit-{sequence}"));
    std::fs::write(&bundle_path, &bundle)
        .map_err(|error| format!("write fake Draw.io bundle: {error}"))?;
    let source = r#"<mxGraphModel page="0"><root><mxCell id="shape" parent="1" vertex="1"><mxGeometry width="10" height="10" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let svg = DrawioJsRuntimeOps::render(source, &bundle_path, DiagramColorPreset::dark())
        .map_err(|error| format!("render fake Draw.io bundle: {error}; bundle: {bundle}"))?;
    parse_box_attribute(&svg)
}

fn path_box_bundle(path_data: &str) -> Result<String, String> {
    let path_literal = serde_json::to_string(path_data)
        .map_err(|error| format!("serialize path data: {error}"))?;
    let mut bundle = PATH_BOX_BUNDLE_PREFIX.to_string();
    bundle.push_str(&path_literal);
    bundle.push_str(
        r#");
  path.setAttribute("fill", "none");
  svg.appendChild(path);
  const box = path.getBBox();
  svg.setAttribute("data-path-box", JSON.stringify([box.x, box.y, box.x + box.width, box.y + box.height]));
  callback({ graph: { getSvg() { return svg; } } });
};"#,
    );
    Ok(bundle)
}

fn parse_box_attribute(svg: &str) -> Result<[f64; PATH_BOX_COORDINATES], String> {
    let (_, value) = svg
        .split_once("data-path-box=\"")
        .ok_or_else(|| format!("bbox attribute missing from {svg}"))?;
    let (value, _) = value
        .split_once('"')
        .ok_or_else(|| format!("bbox attribute is unterminated: {svg}"))?;
    let box_: [f64; PATH_BOX_COORDINATES] = serde_json::from_str(value)
        .map_err(|error| format!("invalid four-coordinate path box: {error}"))?;
    Ok(box_)
}

fn assert_path_box_matches_usvg(
    path_data: &str,
    actual: [f64; PATH_BOX_COORDINATES],
) -> Result<(), String> {
    let markup = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1200\" height=\"900\"><path d=\"{path_data}\" fill=\"black\"/></svg>"
    );
    let tree = usvg::Tree::from_str(&markup, &usvg::Options::default())
        .map_err(|error| format!("parse SVG path: {error}"))?;
    let path = first_path(tree.root().children())
        .ok_or_else(|| format!("usvg path node missing: {path_data}"))?;
    let rect = path.abs_bounding_box();
    assert_box_close(
        actual,
        [
            f64::from(rect.x()),
            f64::from(rect.y()),
            f64::from(rect.right()),
            f64::from(rect.bottom()),
        ],
    )
}

fn first_path(nodes: &[usvg::Node]) -> Option<&usvg::Path> {
    nodes.iter().find_map(|node| match node {
        usvg::Node::Path(path) => Some(path.as_ref()),
        usvg::Node::Group(group) => first_path(group.children()),
        usvg::Node::Image(_) | usvg::Node::Text(_) => None,
    })
}

fn assert_box_close(
    actual_box: [f64; PATH_BOX_COORDINATES],
    expected_box: [f64; PATH_BOX_COORDINATES],
) -> Result<(), String> {
    for (actual, expected) in actual_box.into_iter().zip(expected_box) {
        if !actual.is_finite() || !expected.is_finite() || (actual - expected).abs() >= 0.0001 {
            return Err(format!(
                "{actual} != {expected}; actual box {actual_box:?}; expected box {expected_box:?}"
            ));
        }
    }
    Ok(())
}
