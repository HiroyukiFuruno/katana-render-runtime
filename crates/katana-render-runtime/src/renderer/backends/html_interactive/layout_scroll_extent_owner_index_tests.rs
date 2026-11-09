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

#[test]
fn nonfinite_element_and_inline_fragment_propagate_extent_errors() -> Result<(), String> {
    use super::super::super::types::InlineFragment;

    let mut renderer = owner_renderer();
    let valid = [(1.0, 2.0), (4.0, 2.0), (4.0, 6.0), (1.0, 6.0)];
    renderer.element_boxes[0].transformed_corners = valid;
    renderer.element_boxes[0]
        .inline_fragments
        .push(InlineFragment::new(1.0, 2.0, 3.0, 4.0));
    let indices = HashMap::from([(1, 0)]);
    let measure = |renderer: &HtmlLayoutRenderer| {
        super::super::scrollable_extent(&renderer.element_boxes, &indices, 10.0, 8.0, 0.0)
    };
    assert_eq!(measure(&renderer), Ok((10.0, 8.0)));
    renderer.element_boxes[0].transformed_corners[0].0 = f32::NAN;
    assert_eq!(
        measure(&renderer).err().as_deref(),
        Some(NON_FINITE_GEOMETRY_ERROR)
    );
    renderer.element_boxes[0].transformed_corners = valid;
    renderer.element_boxes[0].inline_fragments[0].transformed_corners[0].1 = f32::INFINITY;
    assert_eq!(
        measure(&renderer).err().as_deref(),
        Some(NON_FINITE_GEOMETRY_ERROR)
    );
    Ok(())
}

#[test]
fn rotated_subject_touching_clip_edge_has_no_area_or_scroll_extent() -> Result<(), String> {
    const RIGHT_EDGE: f32 = 10.0;
    const DIAMOND_MIDDLE_X: f32 = 12.0;
    const DIAMOND_RIGHT_X: f32 = 14.0;
    let subject = [
        (RIGHT_EDGE, 4.0),
        (DIAMOND_MIDDLE_X, 2.0),
        (DIAMOND_RIGHT_X, 4.0),
        (DIAMOND_MIDDLE_X, 6.0),
    ];
    let mut renderer = owner_renderer();
    renderer.element_boxes[0].transformed_corners =
        [(0.0, 0.0), (RIGHT_EDGE, 0.0), (RIGHT_EDGE, 8.0), (0.0, 8.0)];
    renderer.element_boxes[0].overflow_clip_axes = (true, true);
    let indices = HashMap::from([(1, 0)]);
    let clip = clip(RIGHT_EDGE, 8.0);
    assert_edge_contact(&subject, &clip, &renderer, &indices)?;
    let extent = super::super::visible_extent(
        &subject,
        &[clip],
        &renderer.element_boxes,
        &indices,
        &mut PolygonScratch::default(),
    )?;
    assert_eq!(extent, None);
    Ok(())
}

fn assert_edge_contact(
    subject: &[(f32, f32); RECT_CORNER_COUNT],
    clip: &OverflowClip,
    renderer: &HtmlLayoutRenderer,
    indices: &HashMap<u64, usize>,
) -> Result<(), String> {
    let mut scratch = PolygonScratch {
        first: subject
            .iter()
            .map(|&(x, y)| (f64::from(x), f64::from(y)))
            .collect(),
        second: Vec::new(),
    };
    let mut first = true;
    assert!(polygon::clip_owner_polygon(
        clip,
        &renderer.element_boxes,
        indices,
        &mut scratch,
        &mut first
    )?);
    assert!(!scratch.current(first).is_empty());
    assert_eq!(
        axis_aligned::polygon_right_bottom(scratch.current(first))?,
        None
    );
    Ok(())
}
