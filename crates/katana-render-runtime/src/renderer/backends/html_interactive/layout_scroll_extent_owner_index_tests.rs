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

const DIAMOND_CORNERS: [(f32, f32); RECT_CORNER_COUNT] =
    [(5.0, 0.0), (10.0, 5.0), (5.0, 10.0), (0.0, 5.0)];

#[test]
fn rotated_owner_and_viewport_clip_preserve_diamond_extent() -> Result<(), String> {
    let mut rotated = clip(10.0, 10.0);
    rotated.transformed_corners = DIAMOND_CORNERS;
    let input = viewport(-5.0, -5.0, 20.0, 20.0);
    let mut renderer = owner_renderer();
    renderer.element_boxes[0].transformed_corners = DIAMOND_CORNERS;
    renderer.element_boxes[0].overflow_clip_axes = (true, true);
    let indices = HashMap::from([(1, 0)]);
    let mut scratch = PolygonScratch {
        first: input.to_vec(),
        second: Vec::new(),
    };
    let mut first = true;
    assert!(polygon::clip_owner_polygon(
        &rotated,
        &renderer.element_boxes,
        &indices,
        &mut scratch,
        &mut first,
    )?);
    assert_diamond_extent(scratch.current(first))?;
    assert_diamond_extent(&polygon::clip_viewport_polygon(&rotated, &input)?)?;
    assert_axis_aligned_subject_extent(&rotated, &renderer, &indices)
}

fn assert_axis_aligned_subject_extent(
    rotated: &OverflowClip,
    renderer: &HtmlLayoutRenderer,
    indices: &HashMap<u64, usize>,
) -> Result<(), String> {
    const SUBJECT_EDGE: f32 = 12.0;
    let subject = [
        (0.0, 0.0),
        (SUBJECT_EDGE, 0.0),
        (SUBJECT_EDGE, SUBJECT_EDGE),
        (0.0, SUBJECT_EDGE),
    ];
    let extent = super::super::visible_extent(
        &subject,
        std::slice::from_ref(rotated),
        &renderer.element_boxes,
        indices,
        &mut PolygonScratch::default(),
    )?;
    assert_eq!(extent, Some((10.0, 10.0)));
    Ok(())
}

fn assert_diamond_extent(result: &[(f64, f64)]) -> Result<(), String> {
    const EPSILON: f64 = 0.000_001;
    assert_eq!(result.len(), RECT_CORNER_COUNT);
    for (x, y) in DIAMOND_CORNERS {
        assert!(
            result
                .iter()
                .any(|point| (point.0 - f64::from(x)).abs() < EPSILON
                    && (point.1 - f64::from(y)).abs() < EPSILON)
        );
    }
    let (right, bottom) =
        axis_aligned::polygon_right_bottom(result)?.ok_or("rotated clip lost its extent")?;
    assert!((f64::from(right) - 10.0).abs() < EPSILON);
    assert!((f64::from(bottom) - 10.0).abs() < EPSILON);
    Ok(())
}
