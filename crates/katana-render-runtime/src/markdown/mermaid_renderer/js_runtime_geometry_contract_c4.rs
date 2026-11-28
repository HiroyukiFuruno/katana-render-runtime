use super::*;

fn container<'a>(
    root: Node<'a, 'a>,
    viewport: Bounds,
    name: &str,
    label: &str,
    technology: &str,
) -> Result<Bounds, String> {
    let suffix = format!("-{name}");
    let node = unique(root.descendants().filter(|item| {
        class(*item, "c4-container") && item.attribute("id").is_some_and(|id| id.ends_with(&suffix))
    }))?;
    let name_group = unique(node.descendants().filter(|item| class(*item, "c4-name")))?;
    let type_group = unique(node.descendants().filter(|item| class(*item, "c4-type")))?;
    require(text(name_group) == label, "C4 label belongs to wrong node")?;
    require(
        text(type_group).contains(technology),
        "C4 technology belongs to wrong node",
    )?;
    let rect = unique(
        node.children()
            .filter(|item| item.has_tag_name("rect") && class(*item, "label-container")),
    )?;
    let bounds = rectangle(rect)?;
    label_in_box(node, bounds)?;
    view_contains(viewport, bounds)?;
    Ok(bounds)
}

fn boundary(bounds: Bounds, point: Point) -> bool {
    contains(bounds, point)
        && ([bounds[0], bounds[RIGHT_BOUND_INDEX]]
            .iter()
            .any(|x| (point[0] - x).abs() <= SERIALIZED_TOLERANCE)
            || [bounds[1], bounds[BOTTOM_BOUND_INDEX]]
                .iter()
                .any(|y| (point[1] - y).abs() <= SERIALIZED_TOLERANCE))
}

fn relation(root: Node<'_, '_>, a: Bounds, b: Bounds, label: &str) -> Result<(), String> {
    let line = unique(
        root.descendants()
            .filter(|item| item.has_tag_name("line") && item.attribute("marker-end").is_some()),
    )?;
    let start = global(line, [number(line, "x1", None)?, number(line, "y1", None)?])?;
    let end = global(line, [number(line, "x2", None)?, number(line, "y2", None)?])?;
    require(
        boundary(a, start) && boundary(b, end) && !near(start, end),
        "C4 relation endpoints detached or reversed",
    )?;
    for expected in [label, "JSON/HTTPS"] {
        let node = unique(
            root.descendants()
                .filter(|item| item.has_tag_name("text") && text(*item).contains(expected)),
        )?;
        let point = label_anchor(node)?;
        require(
            point[0] >= start[0].min(end[0])
                && point[0] <= start[0].max(end[0])
                && point[1] >= a[1].min(b[1])
                && point[1] <= a[BOTTOM_BOUND_INDEX].max(b[BOTTOM_BOUND_INDEX]),
            "C4 relation label detached from edge corridor",
        )?;
    }
    Ok(())
}

const DYNAMIC_LABEL_COUNT: usize = 5;

fn dynamic_labels(japanese: bool) -> [&'static str; DYNAMIC_LABEL_COUNT] {
    if japanese {
        [
            "シングルページアプリケーション",
            "APIアプリケーション",
            "利用する",
            "JavaScript と React",
            "Java と Spring Boot",
        ]
    } else {
        [
            "Single Page Application",
            "API Application",
            "Uses",
            "javascript and react",
            "Java and Spring Boot",
        ]
    }
}

fn dynamic_title(root: Node<'_, '_>, viewport: Bounds, japanese: bool) -> Result<(), String> {
    let expected = if japanese {
        "APIアプリケーションのダイナミック図"
    } else {
        "Dynamic diagram for API Application"
    };
    source_title(root, viewport, expected)
}

pub(super) fn source_title(
    root: Node<'_, '_>,
    viewport: Bounds,
    expected: &str,
) -> Result<(), String> {
    let title = unique(
        root.descendants()
            .filter(|node| node.has_tag_name("text") && text(*node) == expected),
    )?;
    let anchor = label_anchor(title)?;
    view_contains(viewport, [anchor[0], anchor[1], anchor[0], anchor[1]])
}

impl GeometryContract {
    pub(crate) fn dynamic(svg: &str, japanese: bool) -> Result<(), String> {
        let (doc, viewport) = document(svg)?;
        let root = doc.root_element();
        require(
            root.descendants()
                .filter(|item| class(*item, "c4-container"))
                .count()
                == 2,
            "C4 node inventory changed",
        )?;
        dynamic_title(root, viewport, japanese)?;
        let [spa, api, label, spa_technology, api_technology] = dynamic_labels(japanese);
        let a = container(root, viewport, "spa", spa, spa_technology)?;
        let b = container(root, viewport, "api", api, api_technology)?;
        require(
            a[RIGHT_BOUND_INDEX] < b[0],
            "source nodes overlap or collapse",
        )?;
        relation(root, a, b, label)
    }

    pub(crate) fn deployment(svg: &str) -> Result<(), String> {
        let (doc, viewport) = document(svg)?;
        let root = doc.root_element();
        source_title(
            root,
            viewport,
            "Deployment diagram for Internet Banking System",
        )?;
        let inner = container(root, viewport, "mobile", "Mobile App", "Xamarin")?;
        let (label, technology) = Self::deployment_labels(root)?;
        let outer = Self::deployment_parent(root, inner)?;
        require(
            contains(outer, label_anchor(label)?) && contains(outer, label_anchor(technology)?),
            "deployment labels detached from parent",
        )?;
        view_contains(viewport, outer)
    }

    fn deployment_labels<'a>(root: Node<'a, 'a>) -> Result<(Node<'a, 'a>, Node<'a, 'a>), String> {
        let label = unique(root.descendants().filter(|item| {
            item.has_tag_name("text") && text(*item) == "Customer's mobile device"
        }))?;
        let technology =
            unique(root.descendants().filter(|item| {
                item.has_tag_name("text") && text(*item) == "[Apple iOS or Android]"
            }))?;
        Ok((label, technology))
    }

    fn deployment_parent(root: Node<'_, '_>, inner: Bounds) -> Result<Bounds, String> {
        root.descendants()
            .filter(|item| item.has_tag_name("rect"))
            .filter_map(|item| rectangle(item).ok())
            .find(|bounds| {
                contains(*bounds, [inner[0], inner[1]])
                    && contains(
                        *bounds,
                        [inner[RIGHT_BOUND_INDEX], inner[BOTTOM_BOUND_INDEX]],
                    )
                    && bounds[RIGHT_BOUND_INDEX] - bounds[0] > inner[RIGHT_BOUND_INDEX] - inner[0]
            })
            .ok_or_else(|| "deployment parent geometry missing".to_owned())
    }
}
