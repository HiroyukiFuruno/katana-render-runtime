use super::HtmlLayoutRenderer;
use super::deferred_clip_tests::assert_clip_owner;
use super::stacking_context_tests::{element, pixel, rasterize_layout, render_pixels};
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use crate::renderer::backends::html_interactive::style::CssStyle;
use crate::renderer::backends::html_interactive::types::DetailsContext;
use std::collections::HashMap;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const CLIP_OWNER_NODE_ID: u64 = 2;
const OPACITY_CONTEXT_NODE_ID: u64 = 3;
const FIXED_CHILD_NODE_ID: u64 = 4;
const PAINTED_PIXEL_COORDINATE: u32 = 70;
const RGBA_CHANNEL_COUNT: usize = 4;
const TEST_VIEWPORT_EDGE: u32 = 160;

#[test]
fn mixed_opacity_context_keeps_explicit_z_fixed_child_unclipped() -> TestResult {
    assert_mixed_fixed_clip(";z-index:1")
}

#[test]
fn mixed_opacity_context_keeps_auto_z_fixed_child_unclipped() -> TestResult {
    assert_mixed_fixed_clip("")
}

#[test]
fn fixed_and_sticky_auto_positions_create_stacking_contexts() {
    let mut renderer = renderer();
    for (position, expected_context) in [
        ("fixed", true),
        ("sticky", true),
        ("relative", false),
        ("absolute", false),
    ] {
        let style = CssStyle::from_attributes(
            &[("style".into(), format!("position:{position}"))],
            &CssStyle::browser_default(),
        );
        let start = renderer.deferred_paint.len();
        let context = renderer.begin_stacking_paint_context(OPACITY_CONTEXT_NODE_ID, &style);
        assert_eq!(context.is_some(), expected_context, "position={position}");
        if expected_context {
            assert!(
                renderer
                    .take_stacking_paint_context(OPACITY_CONTEXT_NODE_ID, context)
                    .is_some()
            );
        }
        assert_eq!(renderer.deferred_paint.len(), start);
    }
}

fn assert_mixed_fixed_clip(z_index: &str) -> TestResult {
    for style in [
        "position:relative;width:40px;height:40px;opacity:.5",
        "position:relative;width:40px;height:40px;opacity:.5;z-index:2",
    ] {
        assert_fixed_child_for_style(style, z_index)?;
    }
    Ok(())
}

fn assert_fixed_child_for_style(style: &str, z_index: &str) -> TestResult {
    let fixed = element(
        FIXED_CHILD_NODE_ID,
        &format!(
            "position:fixed;left:60px;top:60px;width:20px;height:20px;background:blue{z_index}"
        ),
        vec![],
    );
    let context = element(OPACITY_CONTEXT_NODE_ID, style, vec![fixed]);
    let clip = element(
        CLIP_OWNER_NODE_ID,
        "position:relative;width:40px;height:40px;overflow:hidden",
        vec![context],
    );
    let root = element(1, "position:relative;width:100px;height:100px", vec![clip]);
    let (layout, pixels) = render_pixels(root)?;
    assert_clip_owner(&layout, OPACITY_CONTEXT_NODE_ID, CLIP_OWNER_NODE_ID, true)?;
    assert_clip_owner(&layout, FIXED_CHILD_NODE_ID, CLIP_OWNER_NODE_ID, false)?;
    assert_half_blue(pixel(
        &pixels,
        PAINTED_PIXEL_COORDINATE,
        PAINTED_PIXEL_COORDINATE,
    )?);
    assert_eq!(layout.svg.matches("opacity=\"0.5\"").count(), 1);
    Ok(())
}

fn assert_half_blue(color: [u8; RGBA_CHANNEL_COUNT]) {
    assert_eq!(color[2..], [255, 255]);
    assert!((126..=129).contains(&color[0]));
    assert!((126..=129).contains(&color[1]));
}

#[test]
fn context_orders_all_real_render_batches_by_z_and_source_order() -> TestResult {
    let mut renderer = renderer();
    let style = CssStyle::from_attributes(
        &[("style".into(), "opacity:.5".into())],
        &CssStyle::browser_default(),
    );
    let scope = renderer.begin_element_paint_scope(1, &style);
    render_batch(&mut renderer, 2, "left:10px;background:red;z-index:9");
    render_batch(&mut renderer, 3, "left:10px;background:blue;z-index:1");
    render_batch(&mut renderer, 4, "left:30px;background:lime;z-index:9");
    renderer.finish_element_paint_scope(scope, 1, &style);
    assert!(renderer.ownership.paint_contexts.is_empty());
    assert!(renderer.ownership.rendering_elements.is_empty());
    let layout = renderer.into_layout_result(100.0)?;
    let pixels = rasterize_layout(&layout)?;
    let red = pixel(&pixels, 20, 20)?;
    let green = pixel(&pixels, 40, 20)?;
    assert_eq!(red[0], 255);
    assert!((126..=129).contains(&red[1]));
    assert!((126..=129).contains(&red[2]));
    assert_eq!(green[1], 255);
    assert!((126..=129).contains(&green[0]));
    assert!((126..=129).contains(&green[2]));
    Ok(())
}

fn render_batch(renderer: &mut HtmlLayoutRenderer, node_id: u64, style: &str) {
    let node = element(
        node_id,
        &format!("position:absolute;top:10px;width:30px;height:30px;{style}"),
        vec![],
    );
    renderer.render_node(
        &node,
        0.0,
        0.0,
        TEST_VIEWPORT_EDGE as f32,
        &CssStyle::browser_default(),
        DetailsContext::NONE,
    );
    renderer.capture_stacking_context_children();
}

pub(super) fn renderer() -> HtmlLayoutRenderer {
    HtmlLayoutRenderer::new(
        HtmlBrowserViewport {
            width: TEST_VIEWPORT_EDGE,
            height: TEST_VIEWPORT_EDGE,
            device_scale_factor: 1.0,
        },
        0.0,
        &HashMap::new(),
        None,
    )
}
