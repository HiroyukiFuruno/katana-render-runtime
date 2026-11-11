use resvg::usvg::roxmltree::{Document, Node};
use svgtypes::{SimplePathSegment, SimplifyingPathParser, Transform, ViewBox};

#[path = "js_runtime_geometry_contract_architecture.rs"]
mod architecture;
#[path = "js_runtime_geometry_contract_c4.rs"]
mod c4;
#[cfg(test)]
#[path = "js_runtime_coverage_gap_geometry_tests.rs"]
mod coverage_gap_tests;
#[path = "js_runtime_geometry_contract_other.rs"]
mod other;
#[path = "js_runtime_geometry_contract_tests.rs"]
mod tests;
type Point = [f64; 2];
const BOUNDS_COMPONENT_COUNT: usize = 4;
type Bounds = [f64; BOUNDS_COMPONENT_COUNT];
const RIGHT_BOUND_INDEX: usize = 2;
const BOTTOM_BOUND_INDEX: usize = 3;
const SERIALIZED_TOLERANCE: f64 = 0.00001;

pub(crate) struct GeometryContract;

fn require(valid: bool, message: &str) -> Result<(), String> {
    valid.then_some(()).ok_or_else(|| message.to_owned())
}

fn attr<'a>(node: Node<'a, 'a>, name: &str) -> Result<&'a str, String> {
    node.attribute(name)
        .ok_or_else(|| format!("missing {name}"))
}

fn number(node: Node<'_, '_>, name: &str, default: Option<f64>) -> Result<f64, String> {
    let value = match node.attribute(name) {
        Some(value) => value.parse::<f64>().map_err(|error| error.to_string())?,
        None => default.ok_or_else(|| format!("missing {name}"))?,
    };
    require(value.is_finite(), "non-finite geometry")?;
    Ok(value)
}

fn class(node: Node<'_, '_>, token: &str) -> bool {
    node.attribute("class")
        .is_some_and(|value| value.split_whitespace().any(|item| item == token))
}

fn text(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(|item| item.is_text())
        .filter_map(|item| item.text())
        .collect()
}

fn unique<'a>(nodes: impl Iterator<Item = Node<'a, 'a>>) -> Result<Node<'a, 'a>, String> {
    let found: Vec<_> = nodes.collect();
    require(found.len() == 1, "semantic element must be unique")?;
    Ok(found[0])
}

fn document(svg: &str) -> Result<Document<'_>, String> {
    let doc = Document::parse(svg).map_err(|error| error.to_string())?;
    let root = doc.root_element();
    require(root.tag_name().name() == "svg", "SVG root required")?;
    /* WHY: svgtypesが4成分の非有限値と、幅・高さの非正値を構文解析で拒否する。 */
    attr(root, "viewBox")?
        .parse::<ViewBox>()
        .map_err(|error| error.to_string())?;
    Ok(doc)
}

fn visible(node: Node<'_, '_>) -> Result<(), String> {
    for ancestor in node.ancestors().filter(|item| item.is_element()) {
        require(
            ancestor.attribute("display") != Some("none")
                && ancestor.attribute("visibility") != Some("hidden")
                && ancestor.attribute("opacity") != Some("0"),
            "semantic geometry is hidden",
        )?;
    }
    Ok(())
}

fn global(node: Node<'_, '_>, mut point: Point) -> Result<Point, String> {
    visible(node)?;
    for ancestor in node.ancestors().filter(|item| item.is_element()) {
        if let Some(value) = ancestor.attribute("transform") {
            let matrix: Transform = value
                .parse()
                .map_err(|error: svgtypes::Error| error.to_string())?;
            point = [
                matrix.a * point[0] + matrix.c * point[1] + matrix.e,
                matrix.b * point[0] + matrix.d * point[1] + matrix.f,
            ];
        }
    }
    require(
        point.iter().all(|value| value.is_finite()),
        "finite transformed geometry required",
    )?;
    Ok(point)
}

fn rectangle(node: Node<'_, '_>) -> Result<Bounds, String> {
    let x = number(node, "x", Some(0.0))?;
    let y = number(node, "y", Some(0.0))?;
    let w = number(node, "width", None)?;
    let h = number(node, "height", None)?;
    require(w > 0.0 && h > 0.0, "semantic rectangle must have area")?;
    let points = [[x, y], [x + w, y], [x, y + h], [x + w, y + h]].map(|p| global(node, p));
    let points = points.into_iter().collect::<Result<Vec<_>, _>>()?;
    let bounds = [
        points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min),
        points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min),
        points
            .iter()
            .map(|p| p[0])
            .fold(f64::NEG_INFINITY, f64::max),
        points
            .iter()
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max),
    ];
    require(
        bounds[RIGHT_BOUND_INDEX] > bounds[0] && bounds[BOTTOM_BOUND_INDEX] > bounds[1],
        "transformed rectangle collapsed",
    )?;
    Ok(bounds)
}

fn contains(bounds: Bounds, point: Point) -> bool {
    point[0] >= bounds[0] - SERIALIZED_TOLERANCE
        && point[0] <= bounds[RIGHT_BOUND_INDEX] + SERIALIZED_TOLERANCE
        && point[1] >= bounds[1] - SERIALIZED_TOLERANCE
        && point[1] <= bounds[BOTTOM_BOUND_INDEX] + SERIALIZED_TOLERANCE
}

fn view_contains(root: Node<'_, '_>, bounds: Bounds) -> Result<(), String> {
    let view: ViewBox = attr(root, "viewBox")?
        .parse()
        .map_err(|error: svgtypes::ViewBoxError| error.to_string())?;
    let outer = [view.x, view.y, view.x + view.w, view.y + view.h];
    require(
        contains(outer, [bounds[0], bounds[1]])
            && contains(
                outer,
                [bounds[RIGHT_BOUND_INDEX], bounds[BOTTOM_BOUND_INDEX]],
            ),
        "semantic geometry outside viewBox",
    )
}

fn label_anchor(node: Node<'_, '_>) -> Result<Point, String> {
    require(!text(node).is_empty(), "associated label must contain text")?;
    global(
        node,
        [number(node, "x", Some(0.0))?, number(node, "y", Some(0.0))?],
    )
}

fn endpoints(node: Node<'_, '_>) -> Result<(Point, Point), String> {
    let mut points = Vec::new();
    for segment in SimplifyingPathParser::from(attr(node, "d")?) {
        match segment.map_err(|error| error.to_string())? {
            SimplePathSegment::MoveTo { x, y }
            | SimplePathSegment::LineTo { x, y }
            | SimplePathSegment::CurveTo { x, y, .. }
            | SimplePathSegment::Quadratic { x, y, .. } => points.push(global(node, [x, y])?),
            SimplePathSegment::ClosePath => (),
        }
    }
    let [first, .., last] = points.as_slice() else {
        return Err("edge must have start and end".to_owned());
    };
    require(!near(*first, *last), "edge must not collapse")?;
    Ok((*first, *last))
}

fn near(a: Point, b: Point) -> bool {
    (a[0] - b[0]).abs() <= SERIALIZED_TOLERANCE && (a[1] - b[1]).abs() <= SERIALIZED_TOLERANCE
}

fn label_in_box(node: Node<'_, '_>, bounds: Bounds) -> Result<(), String> {
    let labels: Vec<_> = node
        .descendants()
        .filter(|item| item.has_tag_name("text"))
        .collect();
    require(!labels.is_empty(), "associated text element required")?;
    for label in labels {
        require(
            contains(bounds, label_anchor(label)?),
            "label anchor detached from node geometry",
        )?;
    }
    Ok(())
}
