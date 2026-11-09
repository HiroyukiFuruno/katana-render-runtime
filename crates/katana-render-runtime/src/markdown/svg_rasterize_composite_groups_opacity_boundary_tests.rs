use super::super::super::tests::{FailingTextRasterizer, TEXT_RASTERIZER_FAILURE};
use super::{AuthoredClipInfo, FADED_OPACITY, SvgCompositeRenderer, opacity_svg, render_text};
use resvg::usvg;
use tiny_skia::{Color, Pixmap, Transform};

const OFFSCREEN_TRANSLATE_X: u32 = 1000;
const ORIGIN_TRANSLATE_Y: u32 = 0;

#[test]
fn offscreen_opacity_group_has_no_crop_or_text_work() -> Result<(), String> {
    let transform = format!("translate({OFFSCREEN_TRANSLATE_X} {ORIGIN_TRANSLATE_Y})");
    let rendered = render_text(FADED_OPACITY, None, Some(&transform))?;
    assert_eq!(rendered.calls, 0);
    assert_white(&rendered.pixmap);
    assert_group_crop_is_none(&transform)
}

#[test]
fn text_rasterizer_error_propagates_without_compositing_group_shapes() -> Result<(), String> {
    let svg = opacity_svg(FADED_OPACITY, None, None, false, true);
    let options = super::super::super::super::font::html_rasterizer_options(&svg);
    let tree = usvg::Tree::from_str(&svg, &options).map_err(|error| error.to_string())?;
    let mut pixmap = Pixmap::new(super::WIDTH, super::HEIGHT).ok_or("pixmap allocation failed")?;
    pixmap.fill(Color::WHITE);
    let rasterizer = FailingTextRasterizer::default();
    let result = SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut pixmap.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    );
    assert_eq!(result.err().as_deref(), Some(TEXT_RASTERIZER_FAILURE));
    assert_eq!(rasterizer.calls(), 1);
    assert_white(&pixmap);
    Ok(())
}

fn assert_white(pixmap: &Pixmap) {
    assert!(pixmap.data().iter().all(|channel| *channel == u8::MAX));
}

fn assert_group_crop_is_none(transform: &str) -> Result<(), String> {
    let svg = opacity_svg(FADED_OPACITY, None, Some(transform), false, true);
    let options = super::super::super::super::font::html_rasterizer_options(&svg);
    let tree = usvg::Tree::from_str(&svg, &options).map_err(|error| error.to_string())?;
    let group = tree
        .root()
        .children()
        .iter()
        .find_map(|node| match node {
            usvg::Node::Group(group) => Some(group.as_ref()),
            _ => None,
        })
        .ok_or("opacity group missing")?;
    let mut pixmap = Pixmap::new(super::WIDTH, super::HEIGHT).ok_or("pixmap allocation failed")?;
    let crop = super::super::super::layers::crop_context(
        group,
        Transform::identity().pre_concat(group.transform()),
        &pixmap.as_mut(),
    )?;
    assert!(crop.is_none(), "offscreen group should have no canvas crop");
    Ok(())
}
