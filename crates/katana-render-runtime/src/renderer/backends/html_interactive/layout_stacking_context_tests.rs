use super::HtmlLayoutRenderer;
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use crate::renderer::backends::html_document::HtmlDocumentNode as Node;
use crate::renderer::backends::html_interactive::style::CssStyle;
use crate::renderer::backends::html_interactive::types::{DetailsContext, LayoutResult};
use std::collections::HashMap;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const TEST_VIEWPORT_EDGE: u32 = 160;
const ROTATED_CHILD_PAINT_SAMPLE_X: u32 = 75;
const ROTATED_CHILD_PAINT_SAMPLE_Y: u32 = 15;
const RGBA_CHANNEL_COUNT: usize = 4;

#[test]
fn transformed_context_rotates_deferred_child_paint_and_boxes_once() -> TestResult {
    let child = element(
        2,
        "position:absolute;left:10px;top:20px;width:20px;height:10px;background:red;z-index:1",
        vec![],
    );
    let root = element(
        1,
        "position:relative;width:100px;height:100px;transform:rotate(90deg)",
        vec![child],
    );
    let (layout, pixels) = render_pixels(root)?;
    assert_eq!(
        pixel(
            &pixels,
            ROTATED_CHILD_PAINT_SAMPLE_X,
            ROTATED_CHILD_PAINT_SAMPLE_Y
        )?,
        [255, 0, 0, 255]
    );
    assert_eq!(pixel(&pixels, 15, 25)?, [255, 255, 255, 255]);
    let child = layout
        .element_boxes
        .iter()
        .find(|item| item.node_id == 2)
        .ok_or("child geometry must exist")?;
    assert_eq!(child.transformed_axis_aligned(), (70.0, 10.0, 10.0, 20.0));
    Ok(())
}

#[test]
fn transformed_opacity_context_rotates_and_composites_real_child_paint_once() -> TestResult {
    let (layout, pixels) = render_pixels(transformed_opacity_fixture())?;
    assert_transformed_opacity_pixels(&pixels)?;
    assert_eq!(layout.svg.matches("opacity=\"0.5\"").count(), 1);
    let child_box = layout
        .element_boxes
        .iter()
        .find(|item| item.node_id == 2)
        .ok_or("child geometry must exist")?;
    assert_eq!(
        child_box.transformed_axis_aligned(),
        (70.0, 10.0, 10.0, 20.0)
    );
    Ok(())
}

fn transformed_opacity_fixture() -> Node {
    let child = element(
        2,
        "position:absolute;left:10px;top:20px;width:20px;height:10px;background:red;z-index:1",
        vec![],
    );
    element(
        1,
        "position:relative;width:100px;height:100px;opacity:.5;transform:rotate(90deg)",
        vec![child],
    )
}

fn assert_transformed_opacity_pixels(pixels: &resvg::tiny_skia::Pixmap) -> TestResult {
    let transformed = pixel(
        pixels,
        ROTATED_CHILD_PAINT_SAMPLE_X,
        ROTATED_CHILD_PAINT_SAMPLE_Y,
    )?;
    assert_eq!(transformed[0], 255);
    assert!((126..=129).contains(&transformed[1]));
    assert!((126..=129).contains(&transformed[2]));
    assert_eq!(transformed[3], 255);
    assert_eq!(pixel(pixels, 15, 25)?, [255, 255, 255, 255]);
    Ok(())
}

#[test]
fn opacity_context_composites_overlapping_z_children_as_one_group() -> TestResult {
    let red = element(
        2,
        "position:absolute;left:10px;top:10px;width:30px;height:30px;background:red;z-index:1",
        vec![],
    );
    let blue = element(
        3,
        "position:absolute;left:25px;top:10px;width:30px;height:30px;background:blue;z-index:2",
        vec![],
    );
    let root = element(
        1,
        "position:relative;width:100px;height:100px;opacity:.5",
        vec![red, blue],
    );
    let (layout, pixels) = render_pixels(root)?;
    let overlap = pixel(&pixels, 30, 20)?;
    assert_eq!(overlap[2..], [255, 255]);
    assert!((126..=129).contains(&overlap[0]));
    assert!((126..=129).contains(&overlap[1]));
    assert_eq!(pixel(&pixels, 50, 20)?, overlap);
    assert_eq!(layout.svg.matches("opacity=\"0.5\"").count(), 1);
    Ok(())
}

#[test]
fn zero_rotation_context_keeps_negative_above_background_inside_clip() -> TestResult {
    let negative = element(
        2,
        "position:absolute;left:10px;top:10px;width:120px;height:40px;background:red;z-index:-1",
        vec![],
    );
    let normal = element(3, "width:20px;height:20px;background:blue", vec![]);
    let root = element(
        1,
        "position:relative;width:100px;height:100px;background:green;overflow:hidden;transform:rotate(0deg)",
        vec![negative, normal],
    );
    let (_, pixels) = render_pixels(root)?;
    assert_eq!(pixel(&pixels, 15, 15)?, [0, 0, 255, 255]);
    assert_eq!(pixel(&pixels, 30, 30)?, [255, 0, 0, 255]);
    assert_eq!(pixel(&pixels, 5, 80)?, [0, 128, 0, 255]);
    assert_eq!(pixel(&pixels, 110, 30)?, [255, 255, 255, 255]);
    Ok(())
}

#[test]
fn nested_explicit_context_does_not_escape_later_sibling_layer() -> TestResult {
    let inner = element(
        3,
        "position:absolute;left:10px;top:10px;width:40px;height:40px;background:blue;z-index:100",
        vec![],
    );
    let nested = element(
        2,
        "position:absolute;left:0;top:0;width:80px;height:80px;background:yellow;z-index:1",
        vec![inner],
    );
    let later = element(
        4,
        "position:absolute;left:20px;top:20px;width:40px;height:40px;background:red;z-index:2",
        vec![],
    );
    let root = element(
        1,
        "position:relative;width:100px;height:100px;transform:rotate(0deg)",
        vec![nested, later],
    );
    let (_, pixels) = render_pixels(root)?;
    assert_eq!(pixel(&pixels, 15, 15)?, [0, 0, 255, 255]);
    assert_eq!(pixel(&pixels, 30, 30)?, [255, 0, 0, 255]);
    assert_eq!(pixel(&pixels, 70, 70)?, [255, 255, 0, 255]);
    Ok(())
}

#[test]
fn context_does_not_capture_prior_sibling_deferred_paint() -> TestResult {
    let prior = element(
        2,
        "position:absolute;left:10px;top:10px;width:40px;height:40px;background:red;z-index:9",
        vec![],
    );
    let child = element(
        4,
        "position:absolute;left:10px;top:10px;width:40px;height:40px;background:blue;z-index:1",
        vec![],
    );
    let later = element(
        3,
        "position:absolute;left:0;top:0;width:80px;height:80px;opacity:.5",
        vec![child],
    );
    let root = element(
        1,
        "position:relative;width:100px;height:100px",
        vec![prior, later],
    );
    let (_, pixels) = render_pixels(root)?;
    assert_eq!(pixel(&pixels, 20, 20)?, [255, 0, 0, 255]);
    Ok(())
}

pub(super) fn element(node_id: u64, style: &str, children: Vec<Node>) -> Node {
    Node::Element {
        node_id,
        tag: "div".to_string(),
        attributes: vec![("style".to_string(), style.to_string())],
        children,
    }
}

pub(super) fn render_pixels(
    root: Node,
) -> Result<(LayoutResult, resvg::tiny_skia::Pixmap), Box<dyn std::error::Error>> {
    let layout = render_layout(root)?;
    let pixels = rasterize_layout(&layout)?;
    Ok((layout, pixels))
}

fn render_layout(root: Node) -> Result<LayoutResult, String> {
    let mut renderer = HtmlLayoutRenderer::new(
        HtmlBrowserViewport {
            width: TEST_VIEWPORT_EDGE,
            height: TEST_VIEWPORT_EDGE,
            device_scale_factor: 1.0,
        },
        0.0,
        &HashMap::new(),
        None,
    );
    let bottom = renderer.render_node(
        &root,
        0.0,
        0.0,
        TEST_VIEWPORT_EDGE as f32,
        &CssStyle::browser_default(),
        DetailsContext::NONE,
    );
    assert!(renderer.ownership.paint_contexts.is_empty());
    assert!(renderer.ownership.rendering_elements.is_empty());
    assert!(renderer.ownership.inline_fragment_owners.is_empty());
    renderer.into_layout_result(bottom)
}

pub(super) fn rasterize_layout(
    layout: &LayoutResult,
) -> Result<resvg::tiny_skia::Pixmap, Box<dyn std::error::Error>> {
    let tree = resvg::usvg::Tree::from_str(&layout.svg, &resvg::usvg::Options::default())?;
    let mut pixels = resvg::tiny_skia::Pixmap::new(TEST_VIEWPORT_EDGE, TEST_VIEWPORT_EDGE)
        .ok_or("pixel buffer must fit")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixels.as_mut(),
    );
    Ok(pixels)
}

pub(super) fn pixel(
    pixels: &resvg::tiny_skia::Pixmap,
    x: u32,
    y: u32,
) -> Result<[u8; RGBA_CHANNEL_COUNT], Box<dyn std::error::Error>> {
    let color = pixels.pixel(x, y).ok_or("pixel must exist")?;
    Ok([color.red(), color.green(), color.blue(), color.alpha()])
}
