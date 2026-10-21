use super::stacking_context_tests::{element, pixel, render_pixels};
use crate::renderer::backends::html_interactive::types::LayoutResult;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const RGBA_CHANNEL_COUNT: usize = 4;
const DEFERRED_CHILD_NODE_ID: u64 = 3;

#[test]
fn deferred_z_child_obeys_non_stacking_rounded_overflow_clip() -> TestResult {
    assert_deferred_clip_pixels(
        "position:absolute;left:0;top:0;width:60px;height:60px;background:red;z-index:1",
        "position:relative;width:40px;height:40px;overflow:hidden;border-radius:20px",
        &[
            (20, 20, [255, 0, 0, 255]),
            (2, 2, [255, 255, 255, 255]),
            (50, 20, [255, 255, 255, 255]),
        ],
    )
}

#[test]
fn zero_sized_active_clip_axes_hide_deferred_paint() -> TestResult {
    let cases = [
        ("width:0;height:40px;overflow-x:clip", (1, 10)),
        ("width:40px;height:0;overflow-y:clip", (10, 1)),
        ("width:0;height:0;overflow-x:clip;overflow-y:clip", (1, 1)),
    ];
    for (clip_style, (x, y)) in cases {
        assert_deferred_clip_pixels(
            "position:absolute;left:0;top:0;width:20px;height:20px;background:red;z-index:1",
            &format!("position:relative;{clip_style}"),
            &[(x, y, [255, 255, 255, 255])],
        )?;
    }
    Ok(())
}

#[test]
fn escaping_absolute_and_fixed_paint_avoid_unrelated_clip() -> TestResult {
    let absolute = element(
        3,
        "position:absolute;left:50px;top:10px;width:20px;height:20px;background:red;z-index:1",
        vec![],
    );
    let fixed = element(
        4,
        "position:fixed;left:50px;top:50px;width:20px;height:20px;background:blue;z-index:2",
        vec![],
    );
    let clip = element(
        2,
        "width:40px;height:40px;overflow:hidden",
        vec![absolute, fixed],
    );
    let root = element(1, "position:relative;width:100px;height:100px", vec![clip]);
    let (layout, pixels) = render_pixels(root)?;
    assert_clip_owner(&layout, 3, 2, false)?;
    assert_clip_owner(&layout, 4, 2, false)?;
    assert_eq!(pixel(&pixels, 60, 20)?, [255, 0, 0, 255]);
    assert_eq!(pixel(&pixels, 60, 60)?, [0, 0, 255, 255]);
    Ok(())
}

#[test]
fn shared_clip_does_not_wrap_prior_sibling_deferred_queue() -> TestResult {
    let prior = element(
        2,
        "position:absolute;left:60px;top:10px;width:20px;height:20px;background:red;z-index:2",
        vec![],
    );
    let child = element(
        4,
        "position:absolute;left:0;top:0;width:60px;height:40px;background:blue;z-index:1",
        vec![],
    );
    let clip = element(
        3,
        "position:relative;width:40px;height:40px;overflow:hidden",
        vec![child],
    );
    let root = element(
        1,
        "position:relative;width:100px;height:100px",
        vec![prior, clip],
    );
    let (layout, pixels) = render_pixels(root)?;
    assert_clip_owner(&layout, 2, 3, false)?;
    assert_clip_owner(&layout, 4, 3, true)?;
    assert_eq!(pixel(&pixels, 70, 20)?, [255, 0, 0, 255]);
    assert_eq!(pixel(&pixels, 20, 20)?, [0, 0, 255, 255]);
    assert_eq!(pixel(&pixels, 50, 20)?, [255, 255, 255, 255]);
    Ok(())
}

#[test]
fn ancestor_context_rotates_shared_child_clip_with_paint() -> TestResult {
    let child = element(
        3,
        "position:absolute;left:0;top:0;width:60px;height:60px;background:red;z-index:1",
        vec![],
    );
    let clip = element(
        2,
        "position:relative;width:40px;height:40px;overflow:hidden;border-radius:20px",
        vec![child],
    );
    let root = element(
        1,
        "position:relative;width:100px;height:100px;transform:rotate(90deg)",
        vec![clip],
    );
    let (layout, pixels) = render_pixels(root)?;
    assert_clip_owner(&layout, 3, 2, true)?;
    assert_eq!(pixel(&pixels, 80, 20)?, [255, 0, 0, 255]);
    assert_eq!(pixel(&pixels, 98, 2)?, [255, 255, 255, 255]);
    assert_eq!(pixel(&pixels, 50, 20)?, [255, 255, 255, 255]);
    Ok(())
}

#[test]
fn outer_clip_stays_outside_inner_context_rotation() -> TestResult {
    let child = element(
        4,
        "position:absolute;left:10px;top:10px;width:60px;height:10px;background:red;z-index:1",
        vec![],
    );
    let context = element(
        3,
        "position:relative;width:60px;height:60px;transform:rotate(90deg)",
        vec![child],
    );
    let clip = element(
        2,
        "position:relative;width:45px;height:45px;overflow:hidden",
        vec![context],
    );
    let root = element(1, "position:relative;width:100px;height:100px", vec![clip]);
    let (layout, pixels) = render_pixels(root)?;
    assert_clip_owner(&layout, 4, 2, true)?;
    assert_eq!(pixel(&pixels, 42, 30)?, [255, 0, 0, 255]);
    assert_eq!(pixel(&pixels, 48, 30)?, [255, 255, 255, 255]);
    assert_eq!(pixel(&pixels, 42, 55)?, [255, 255, 255, 255]);
    Ok(())
}

fn assert_deferred_clip_pixels(
    child_style: &str,
    clip_style: &str,
    samples: &[(u32, u32, [u8; RGBA_CHANNEL_COUNT])],
) -> TestResult {
    let child = element(DEFERRED_CHILD_NODE_ID, child_style, vec![]);
    let clip = element(2, clip_style, vec![child]);
    let root = element(1, "position:relative;width:100px;height:100px", vec![clip]);
    let (layout, pixels) = render_pixels(root)?;
    assert_clip_owner(&layout, DEFERRED_CHILD_NODE_ID, 2, true)?;
    for &(x, y, expected) in samples {
        assert_eq!(pixel(&pixels, x, y)?, expected);
    }
    Ok(())
}

pub(super) fn assert_clip_owner(
    layout: &LayoutResult,
    node_id: u64,
    owner: u64,
    expected: bool,
) -> TestResult {
    let target = layout
        .element_boxes
        .iter()
        .find(|item| item.node_id == node_id)
        .ok_or("target geometry must exist")?;
    assert_eq!(
        target
            .overflow_clips
            .iter()
            .any(|clip| clip.owner_node_id == owner),
        expected
    );
    Ok(())
}
