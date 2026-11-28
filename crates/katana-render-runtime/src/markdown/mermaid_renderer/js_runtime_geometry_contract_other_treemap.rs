use super::*;

const EXPECTED_TREEMAP_LEAF_COUNT: usize = 4;

impl GeometryContract {
    fn leaf(
        root: Node<'_, '_>,
        viewport: Bounds,
        name: &str,
        value: &str,
    ) -> Result<Bounds, String> {
        let node = unique(root.descendants().filter(|item| {
            class(*item, "treemapLeafGroup")
                && item
                    .children()
                    .any(|label| class(label, "treemapLabel") && text(label) == name)
        }))?;
        let label = unique(node.children().filter(|item| class(*item, "treemapLabel")))?;
        let amount = unique(node.children().filter(|item| class(*item, "treemapValue")))?;
        require(text(amount) == value, "treemap value belongs to wrong leaf")?;
        let rect = unique(node.children().filter(|item| class(*item, "treemapLeaf")))?;
        let bounds = rectangle(rect)?;
        require(
            contains(bounds, label_anchor(label)?) && contains(bounds, label_anchor(amount)?),
            "treemap label/value detached from leaf",
        )?;
        let style = attr(amount, "style")?;
        require(!style.trim().is_empty(), "treemap value style missing")?;
        view_contains(viewport, bounds)?;
        Ok(bounds)
    }

    pub(crate) fn treemap(svg: &str) -> Result<(), String> {
        let (doc, viewport) = document(svg)?;
        let root = doc.root_element();
        c4::source_title(root, viewport, "Runtime cost")?;
        require(
            root.descendants()
                .filter(|item| class(*item, "treemapLeafGroup"))
                .count()
                == EXPECTED_TREEMAP_LEAF_COUNT,
            "treemap leaf inventory changed",
        )?;
        const TREEMAP_EXPECTED_VALUES: &[(&str, &str)] = &[
            ("Mermaid", "45"),
            ("DOM shim", "25"),
            ("Rasterize", "20"),
            ("Cache", "10"),
        ];
        let bounds = TREEMAP_EXPECTED_VALUES
            .iter()
            .map(|(name, value)| Self::leaf(root, viewport, name, value))
            .collect::<Result<Vec<_>, _>>()?;
        Self::assert_treemap_leaves_do_not_overlap(&bounds)
    }

    fn assert_treemap_leaves_do_not_overlap(bounds: &[Bounds]) -> Result<(), String> {
        for (index, a) in bounds.iter().enumerate() {
            for b in &bounds[index + 1..] {
                require(
                    a[RIGHT_BOUND_INDEX] <= b[0]
                        || b[RIGHT_BOUND_INDEX] <= a[0]
                        || a[BOTTOM_BOUND_INDEX] <= b[1]
                        || b[BOTTOM_BOUND_INDEX] <= a[1],
                    "treemap leaves overlap",
                )?;
            }
        }
        Ok(())
    }
}
