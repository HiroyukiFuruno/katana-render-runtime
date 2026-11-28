use super::deferred_clip_tests::assert_clip_owner;
use super::stacking_context_tests::{element, pixel, render_pixels};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn absolute_auto_escapes_only_clip_outside_containing_block() -> TestResult {
    let child = element(
        3,
        "position:absolute;left:60px;top:60px;width:20px;height:20px;background:red",
        vec![],
    );
    let clip = element(2, "width:40px;height:40px;overflow:hidden", vec![child]);
    let root = element(1, "position:relative;width:100px;height:100px", vec![clip]);
    let (layout, pixels) = render_pixels(root)?;
    assert_clip_owner(&layout, 3, 2, false)?;
    assert_eq!(pixel(&pixels, 70, 70)?, [255, 0, 0, 255]);
    Ok(())
}

#[test]
fn auto_positioned_later_peer_keeps_source_order_in_clipped_lane() -> TestResult {
    for position in ["relative", "absolute"] {
        let fixed = element(
            3,
            "position:fixed;left:10px;top:10px;width:30px;height:30px;background:red",
            vec![],
        );
        let peer = element(
            4,
            &format!(
                "position:{position};left:10px;top:10px;width:30px;height:30px;background:blue"
            ),
            vec![],
        );
        let clip = element(
            2,
            "position:relative;width:40px;height:40px;overflow:hidden",
            vec![fixed, peer],
        );
        let root = element(1, "position:relative;width:100px;height:100px", vec![clip]);
        let (layout, pixels) = render_pixels(root)?;
        assert_clip_owner(&layout, 3, 2, false)?;
        assert_clip_owner(&layout, 4, 2, true)?;
        assert_eq!(pixel(&pixels, 20, 20)?, [0, 0, 255, 255]);
    }
    Ok(())
}

#[test]
fn transformed_nested_opacity_clips_its_fixed_auto_descendant() -> TestResult {
    let child = element(
        4,
        "position:fixed;left:10px;top:10px;width:60px;height:10px;background:blue",
        vec![],
    );
    let inner = element(3, "opacity:.5;width:60px;height:60px", vec![child]);
    let clip = element(
        2,
        "position:relative;width:40px;height:40px;overflow:hidden;transform:rotate(90deg);opacity:.5",
        vec![inner],
    );
    let root = element(1, "position:relative;width:100px;height:100px", vec![clip]);
    let (layout, pixels) = render_pixels(root)?;
    assert_clip_owner(&layout, 4, 2, true)?;
    assert_eq!(layout.svg.matches("opacity=\"0.5\"").count(), 2);
    let inside = pixel(&pixels, 25, 25)?;
    assert_eq!(inside[2..], [255, 255]);
    assert!((190..=193).contains(&inside[0]));
    assert!((190..=193).contains(&inside[1]));
    assert_eq!(pixel(&pixels, 25, 45)?, [255, 255, 255, 255]);
    Ok(())
}

#[test]
fn no_clip_auto_positioned_paint_keeps_existing_string_lane() -> TestResult {
    let child = element(
        2,
        "position:absolute;left:10px;top:10px;width:30px;height:30px;background:red",
        vec![],
    );
    let later = element(3, "width:40px;height:40px;background:blue", vec![]);
    let root = element(
        1,
        "position:relative;width:100px;height:100px",
        vec![child, later],
    );
    let (_, pixels) = render_pixels(root)?;
    assert_eq!(pixel(&pixels, 20, 20)?, [0, 0, 255, 255]);
    Ok(())
}
