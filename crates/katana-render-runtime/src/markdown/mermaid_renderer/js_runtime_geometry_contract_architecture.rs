use super::*;

fn service<'a>(
    root: Node<'a, 'a>,
    viewport: Bounds,
    name: &str,
    expected: &str,
) -> Result<(Node<'a, 'a>, Bounds), String> {
    let suffix = format!("-service-{name}");
    let node = unique(root.descendants().filter(|item| {
        class(*item, "architecture-service")
            && item.attribute("id").is_some_and(|id| id.ends_with(&suffix))
    }))?;
    require(
        text(node) == expected,
        "service label belongs to wrong node",
    )?;
    let icon = unique(node.descendants().filter(|item| item.has_tag_name("svg")))?;
    let bounds = rectangle(icon)?;
    view_contains(viewport, bounds)?;
    label_in_box(node, bounds)?;
    Ok((node, bounds))
}

fn port(bounds: Bounds, side: char) -> Result<Point, String> {
    let middle = [
        (bounds[0] + bounds[RIGHT_BOUND_INDEX]) / 2.0,
        (bounds[1] + bounds[BOTTOM_BOUND_INDEX]) / 2.0,
    ];
    match side {
        'L' => Ok([bounds[0], middle[1]]),
        'R' => Ok([bounds[RIGHT_BOUND_INDEX], middle[1]]),
        'T' => Ok([middle[0], bounds[1]]),
        'B' => Ok([middle[0], bounds[BOTTOM_BOUND_INDEX]]),
        _ => Err("unknown source port".into()),
    }
}

fn relation(
    root: Node<'_, '_>,
    source: &str,
    target: &str,
    sides: (char, char),
    nodes: &[(String, Bounds)],
) -> Result<(), String> {
    let suffix = format!("-L_{source}_{target}_0");
    let path = unique(root.descendants().filter(|item| {
        class(*item, "edge") && item.attribute("id").is_some_and(|id| id.ends_with(&suffix))
    }))?;
    let lookup = |name| {
        nodes
            .iter()
            .find(|(id, _)| id == name)
            .map(|(_, bounds)| *bounds)
            .ok_or_else(|| "edge source/target missing".to_owned())
    };
    let (start, end) = endpoints(path)?;
    let a = port(lookup(source)?, sides.0)?;
    let b = port(lookup(target)?, sides.1)?;
    require(
        (near(start, a) && near(end, b)) || (near(start, b) && near(end, a)),
        "edge endpoints detached from source ports",
    )
}

fn group_bounds(
    root: Node<'_, '_>,
    viewport: Bounds,
    nodes: &[(String, Bounds)],
    group: (&str, &str),
) -> Result<(), String> {
    let suffix = format!("-group-{}", group.0);
    let container = unique(
        root.descendants()
            .filter(|item| item.attribute("id").is_some_and(|id| id.ends_with(&suffix))),
    )?;
    let outer = rectangle(container)?;
    view_contains(viewport, outer)?;
    for (_, bounds) in nodes {
        require(
            contains(outer, [bounds[0], bounds[1]])
                && contains(
                    outer,
                    [bounds[RIGHT_BOUND_INDEX], bounds[BOTTOM_BOUND_INDEX]],
                ),
            "service outside declared group",
        )?;
    }
    require(
        text(container.parent().ok_or("group parent missing")?).contains(group.1),
        "group label missing",
    )
}

fn service_inventory(
    root: Node<'_, '_>,
    viewport: Bounds,
    services: &[(&str, &str)],
) -> Result<Vec<(String, Bounds)>, String> {
    require(
        root.descendants()
            .filter(|item| class(*item, "architecture-service"))
            .count()
            == services.len(),
        "service inventory changed",
    )?;
    let nodes = services
        .iter()
        .map(|(id, label)| service(root, viewport, id, label).map(|(_, b)| (id.to_string(), b)))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, (_, a)) in nodes.iter().enumerate() {
        for (_, b) in &nodes[index + 1..] {
            require(!near([a[0], a[1]], [b[0], b[1]]), "services collapsed")?;
        }
    }
    Ok(nodes)
}

impl GeometryContract {
    pub(crate) fn architecture(
        svg: &str,
        services: &[(&str, &str)],
        edges: &[(&str, &str, char, char)],
        group: (&str, &str),
    ) -> Result<(), String> {
        let (doc, viewport) = document(svg)?;
        let root = doc.root_element();
        let nodes = service_inventory(root, viewport, services)?;
        require(
            root.descendants()
                .filter(|item| class(*item, "edge"))
                .count()
                == edges.len(),
            "source relation inventory changed",
        )?;
        for (source, target, a, b) in edges {
            relation(root, source, target, (*a, *b), &nodes)?;
        }
        group_bounds(root, viewport, &nodes, group)
    }

    pub(crate) fn icons(svg: &str) -> Result<(), String> {
        let (doc, viewport) = document(svg)?;
        let root = doc.root_element();
        let (database, _) = service(root, viewport, "db", "Database")?;
        require(
            database.descendants().any(|node| {
                node.attribute("rx") == Some("20") && node.attribute("ry") == Some("7.14")
            }),
            "database source icon lost",
        )?;
        for name in ["disk1", "disk2"] {
            let (disk, _) = service(root, viewport, name, "Storage")?;
            require(
                disk.descendants().any(|node| {
                    node.attribute("d")
                        .is_some_and(|d| d.contains("l-4.83,13.22"))
                }),
                "disk source icon lost",
            )?;
        }
        Ok(())
    }
}
