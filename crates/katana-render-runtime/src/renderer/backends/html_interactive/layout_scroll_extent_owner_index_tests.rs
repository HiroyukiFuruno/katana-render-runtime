use super::*;

#[test]
fn clip_owner_rejects_missing_out_of_range_and_mismatched_indices() {
    let mut renderer = owner_renderer();
    for indices in [
        HashMap::new(),
        HashMap::from([(1, renderer.element_boxes.len())]),
    ] {
        assert_missing_polygon_owner(&renderer, &indices);
    }
    renderer.element_boxes[0].node_id = 2;
    assert_mismatched_owner_node(&renderer);
}

fn assert_missing_polygon_owner(renderer: &HtmlLayoutRenderer, indices: &HashMap<u64, usize>) {
    let mut first = true;
    assert_eq!(
        polygon::clip_owner_polygon(
            &clip(10.0, 8.0),
            &renderer.element_boxes,
            indices,
            &mut PolygonScratch::default(),
            &mut first,
        )
        .err()
        .as_deref(),
        Some("scrollable overflow clip owner is missing")
    );
}

fn assert_mismatched_owner_node(renderer: &HtmlLayoutRenderer) {
    assert_eq!(
        super::super::clip_owner(
            &clip(10.0, 8.0),
            &renderer.element_boxes,
            &HashMap::from([(1, 0)])
        )
        .err()
        .as_deref(),
        Some("scrollable overflow clip owner is missing")
    );
}
