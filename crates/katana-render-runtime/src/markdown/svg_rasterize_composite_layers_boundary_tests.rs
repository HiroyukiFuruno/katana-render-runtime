use super::super::AuthoredClipInfo;
use resvg::usvg::{Group, Node, Options, Tree};
use tiny_skia::{IntRect, Pixmap, Transform};

const SIZE: u32 = 32;
const ARC_SCALE: f32 = 4.0;
const NESTED_CLIP: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><defs><clipPath id="inner"><rect x="4" y="4" width="8" height="8"/></clipPath><clipPath id="outer" clip-path="url(#inner)"><rect width="16" height="16"/></clipPath></defs><g id="layer" clip-path="url(#outer)"><rect width="24" height="24" fill="#ff0000"/></g></svg>"##;
const PLAIN_GROUP: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4096" height="4096"><g id="layer" opacity="0.5"><rect width="4096" height="4096"/></g></svg>"#;
const ARC_CLIP: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><defs><clipPath id="window"><rect width="32" height="32"/></clipPath></defs><g id="layer" clip-path="url(#window)"><path id="arc" d="M2 12 A10 3 0 0 1 22 12 L2 12Z" fill="red"/></g></svg>"##;
const LARGE_BOUND: f32 = 1.0e38;

#[test]
fn nested_authored_clip_masks_intersect_in_the_real_render_path() -> Result<(), String> {
    let tree = parse(NESTED_CLIP)?;
    let group = layer(&tree)?;
    let mut pixmap = canvas()?;
    super::render_clipped_group(
        group,
        Transform::identity(),
        &mut pixmap.as_mut(),
        &AuthoredClipInfo::default(),
        None,
    )?;
    assert_eq!(pixmap.pixel(6, 6).map(|pixel| pixel.alpha()), Some(255));
    for (x, y) in [(2, 2), (20, 20)] {
        assert_eq!(pixmap.pixel(x, y).map(|pixel| pixel.alpha()), Some(0));
    }
    Ok(())
}

#[test]
fn clipped_group_rejects_empty_transforms_and_off_canvas_layers() -> Result<(), String> {
    let tree = parse(NESTED_CLIP)?;
    let mut pixmap = canvas()?;
    for transform in [
        Transform::from_scale(0.0, 0.0),
        Transform::from_translate(1024.0, 1024.0),
    ] {
        assert!(super::crop_context(layer(&tree)?, transform, &pixmap.as_mut())?.is_none());
        super::render_clipped_group(
            layer(&tree)?,
            transform,
            &mut pixmap.as_mut(),
            &AuthoredClipInfo::default(),
            None,
        )?;
    }
    assert!(pixmap.data().iter().all(|byte| *byte == 0));
    Ok(())
}

#[test]
fn clip_layer_integer_boundaries_fail_before_allocation() -> Result<(), String> {
    let tree = parse(PLAIN_GROUP)?;
    let mut pixmap = canvas()?;
    /* WHY: i32端でもf32の矩形幅を保ち、整数paddingのoverflowをallocation前に検証する。 */
    let oversized = u32::MAX as f32 / 4096.0;
    let cases = [
        (
            Transform::from_translate(i32::MIN as f32, 0.0),
            "x coordinate overflow",
        ),
        (
            Transform::from_translate(0.0, i32::MIN as f32),
            "y coordinate overflow",
        ),
        (Transform::from_scale(oversized, 1.0), "width overflow"),
        (Transform::from_scale(1.0, oversized), "height overflow"),
    ];
    for (transform, expected) in cases {
        let result = super::clipped_group_rect(layer(&tree)?, transform, &pixmap.as_mut());
        assert!(
            result.is_err_and(|error| error.contains(expected)),
            "{expected}"
        );
    }
    Ok(())
}

#[test]
fn cropped_group_without_a_clip_leaves_the_target_unchanged() -> Result<(), String> {
    let tree = parse(PLAIN_GROUP)?;
    let mut pixmap = canvas()?;
    let crop = IntRect::from_xywh(0, 0, SIZE, SIZE).ok_or("crop fixture is invalid")?;
    let info = AuthoredClipInfo::default();
    let context = super::super::RenderContext {
        provenance: &info,
        text: None,
        plan: None,
    };
    super::render_cropped_group(
        layer(&tree)?,
        crop,
        Transform::identity(),
        &mut pixmap.as_mut(),
        &context,
    )?;
    assert!(pixmap.data().iter().all(|byte| *byte == 0));
    Ok(())
}

#[test]
fn clip_mask_rejects_zero_dimensions() -> Result<(), String> {
    let tree = parse(NESTED_CLIP)?;
    let clip = layer(&tree)?
        .clip_path()
        .ok_or("nested clip fixture is missing")?;
    for (width, height) in [(0, SIZE), (SIZE, 0)] {
        let result = super::clip_mask(clip, Transform::identity(), width, height);
        assert!(result.is_err_and(|error| error.contains("allocate cropped SVG clip pixmap")));
    }
    Ok(())
}

#[test]
fn zero_area_path_does_not_render_an_existing_node() -> Result<(), String> {
    let tree = parse(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><path id="empty" d="M1 1H10"/></svg>"#,
    )?;
    let node = tree
        .node_by_id("empty")
        .ok_or("zero-area path fixture is missing")?;
    assert!(node.abs_layer_bounding_box().is_none());
    let mut pixmap = canvas()?;
    super::render_existing_node(node, Transform::identity(), &mut pixmap.as_mut());
    assert!(pixmap.data().iter().all(|byte| *byte == 0));
    Ok(())
}

#[test]
fn invalid_planned_bounds_fall_back_before_any_pixels_are_written() -> Result<(), String> {
    let tree = parse(ARC_CLIP)?;
    let group = layer(&tree)?;
    let plan = super::super::arc_plan::NativeArcPlan::build(&tree)?;
    let Some(Node::Path(path)) = tree.node_by_id("arc") else {
        return Err("arc missing".into());
    };
    assert!(plan.path(path).is_some());
    let extra = resvg::usvg::Rect::from_ltrb(-LARGE_BOUND, -LARGE_BOUND, LARGE_BOUND, LARGE_BOUND)
        .ok_or("planned bounds fixture invalid")?;
    for transform in [Transform::identity(), visible_scaled_arc_transform()] {
        assert!(
            super::super::groups::planned_crop(group, transform, &canvas()?.as_mut(), extra)
                .is_none()
        );
        assert_strict_fallback(group, &plan, transform, extra)?;
    }
    Ok(())
}

fn assert_strict_fallback(
    group: &Group,
    plan: &super::super::arc_plan::NativeArcPlan<'_>,
    transform: Transform,
    extra: resvg::usvg::Rect,
) -> Result<(), String> {
    let info = AuthoredClipInfo::default();
    let text = super::super::tests::FallbackTextRasterizer;
    let context = super::super::RenderContext {
        provenance: &info,
        text: Some(&text),
        plan: Some(plan),
    };
    let mut actual = canvas()?;
    let mut expected = canvas()?;
    super::super::groups::render_clipped_with_extra(
        group,
        transform,
        &mut actual.as_mut(),
        &context,
        Some(extra),
    )?;
    super::render_clipped_group(group, transform, &mut expected.as_mut(), &info, Some(&text))?;
    assert!(expected.data().iter().any(|byte| *byte != 0));
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

fn parse(svg: &str) -> Result<Tree, String> {
    Tree::from_str(svg, &Options::default()).map_err(|error| error.to_string())
}

fn layer(tree: &Tree) -> Result<&Group, String> {
    let Some(Node::Group(group)) = tree.node_by_id("layer") else {
        return Err("layer fixture group is missing".to_owned());
    };
    Ok(group)
}

fn canvas() -> Result<Pixmap, String> {
    Pixmap::new(SIZE, SIZE).ok_or_else(|| "layer fixture canvas allocation failed".to_owned())
}

/* WHY: 4倍拡大時も弧を32pxキャンバス内へ移し、非空fallback対照を維持する。 */
fn visible_scaled_arc_transform() -> Transform {
    let offset = -(SIZE as f32);
    Transform::from_row(ARC_SCALE, 0.0, 0.0, ARC_SCALE, offset, offset)
}
