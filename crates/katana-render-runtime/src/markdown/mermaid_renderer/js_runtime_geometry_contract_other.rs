use super::*;

#[path = "js_runtime_geometry_contract_other_treemap.rs"]
mod treemap;

fn commit<'a>(root: Node<'a, 'a>, name: &str) -> Result<Point, String> {
    let node = unique(root.descendants().filter(|item| {
        item.has_tag_name("circle")
            && class(*item, "commit")
            && class(*item, name)
            && !class(*item, "commit-merge")
    }))?;
    require(
        number(node, "r", None)? > 0.0,
        "commit bullet must have area",
    )?;
    let point = global(node, [number(node, "cx", None)?, number(node, "cy", None)?])?;
    let label = unique(
        root.descendants()
            .filter(|item| class(*item, "commit-label") && text(*item) == name),
    )?;
    let rect = label
        .prev_sibling_element()
        .filter(|rect| class(*rect, "commit-label-bkg"))
        .ok_or("commit label paired background missing")?;
    require(
        contains(rectangle(rect)?, label_anchor(label)?),
        "commit label detached from own background",
    )?;
    view_contains(root, rectangle(rect)?)?;
    nearest_commit(root, rectangle(rect)?, point)?;
    Ok(point)
}

fn nearest_commit(root: Node<'_, '_>, bounds: Bounds, expected: Point) -> Result<(), String> {
    let center = [
        (bounds[0] + bounds[RIGHT_BOUND_INDEX]) / 2.0,
        (bounds[1] + bounds[BOTTOM_BOUND_INDEX]) / 2.0,
    ];
    let distance = |point: Point| (point[0] - center[0]).powi(2) + (point[1] - center[1]).powi(2);
    for node in root
        .descendants()
        .filter(|node| node.has_tag_name("circle") && class(*node, "commit"))
    {
        let point = global(node, [number(node, "cx", None)?, number(node, "cy", None)?])?;
        require(
            distance(expected) <= distance(point) + SERIALIZED_TOLERANCE,
            "commit label nearer a different source commit",
        )?;
    }
    Ok(())
}

fn git_source_relations(
    root: Node<'_, '_>,
    bullets: &[Point],
    base: Point,
    feature: Point,
) -> Result<(), String> {
    let merge: Vec<_> = bullets
        .iter()
        .copied()
        .filter(|p| !near(*p, base) && !near(*p, feature))
        .collect();
    require(merge.len() == 1, "merge commit collapsed or missing")?;
    for (from, to) in [(base, feature), (base, merge[0]), (feature, merge[0])] {
        require(
            GeometryContract::paths_link(root, from, to)?,
            "source commit relationship missing",
        )?;
    }
    for name in ["main", "feature"] {
        unique(
            root.descendants()
                .filter(|node| class(*node, "branchLabel") && text(*node) == name),
        )?;
    }
    Ok(())
}

const EXPECTED_GIT_COMMIT_COUNT: usize = 3;
const EXPECTED_GIT_RELATION_COUNT: usize = 3;

impl GeometryContract {
    pub(crate) fn git_graph(svg: &str) -> Result<(), String> {
        let doc = document(svg)?;
        let root = doc.root_element();
        let base = commit(root, "base")?;
        let feature = commit(root, "rust-js")?;
        require(!near(base, feature), "commit bullets collapsed")?;
        let bullets = root
            .descendants()
            .filter(|item| {
                item.has_tag_name("circle")
                    && class(*item, "commit")
                    && !class(*item, "commit-merge")
            })
            .map(|item| global(item, [number(item, "cx", None)?, number(item, "cy", None)?]))
            .collect::<Result<Vec<_>, _>>()?;
        require(
            bullets.len() == EXPECTED_GIT_COMMIT_COUNT,
            "git commit inventory changed",
        )?;
        Self::git_edges(root, &bullets)?;
        git_source_relations(root, &bullets, base, feature)
    }

    fn git_edges(root: Node<'_, '_>, bullets: &[Point]) -> Result<(), String> {
        for point in bullets {
            view_contains(root, [point[0], point[1], point[0], point[1]])?;
        }
        let paths: Vec<_> = root
            .descendants()
            .filter(|item| item.has_tag_name("path") && class(*item, "arrow"))
            .collect();
        require(
            paths.len() == EXPECTED_GIT_RELATION_COUNT,
            "git relation inventory changed",
        )?;
        for path in paths {
            let (a, b) = endpoints(path)?;
            require(
                bullets.iter().any(|p| near(*p, a)) && bullets.iter().any(|p| near(*p, b)),
                "git relation detached from commit bullets",
            )?;
        }
        Ok(())
    }

    fn paths_link(root: Node<'_, '_>, a: Point, b: Point) -> Result<bool, String> {
        let paths = root
            .descendants()
            .filter(|item| item.has_tag_name("path") && class(*item, "arrow"));
        for path in paths {
            let (x, y) = endpoints(path)?;
            if near(x, a) && near(y, b) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
