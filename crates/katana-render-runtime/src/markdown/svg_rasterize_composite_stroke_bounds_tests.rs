use super::tests::{FallbackTextRasterizer, parse_svg_pixmap_with_bundled_fonts};
use super::{AuthoredClipInfo, SvgCompositeRenderer};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const STROKE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
<path id="horizontal" d="M8 16 H56" fill="none" stroke="#ff0000" stroke-width="2"/>
<path id="vertical" d="M16 8 V56" fill="none" stroke="#00ff00" stroke-width="2"/>
<path id="diagonal" d="M8 8 L56 56" fill="none" stroke="#0000ff" stroke-width="2"/>
<g transform="translate(3 4)"><path id="translated" d="M8 28 H56" fill="none" stroke="#ffffff" stroke-width="2"/></g>
<text x="2" y="62" font-size="10">stroke</text>
</svg>"##;

#[test]
fn axis_aligned_and_transformed_strokes_keep_the_existing_render_contract() -> Result<(), String> {
    let (tree, mut actual) = parse_svg_pixmap_with_bundled_fonts(STROKE_SVG, WIDTH, HEIGHT)?;
    assert_axis_path_bboxes(&tree)?;
    let mut expected = tiny_skia::Pixmap::new(WIDTH, HEIGHT)
        .ok_or_else(|| "failed to allocate stroke expected pixmap".to_string())?;
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut expected.as_mut(),
    );
    SvgCompositeRenderer::render_with_text(
        &tree,
        tiny_skia::Transform::identity(),
        &mut actual.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&FallbackTextRasterizer),
    )?;
    assert_eq!(actual.data(), expected.data());
    assert_nontransparent(&actual, 32, 16)?;
    assert_nontransparent(&actual, 16, 32)?;
    assert_nontransparent(&actual, 40, 40)?;
    assert_nontransparent(&actual, 35, 32)?;
    assert_region_nontransparent(&actual, 0, 57, WIDTH, HEIGHT)?;
    Ok(())
}

fn assert_axis_path_bboxes(tree: &resvg::usvg::Tree) -> Result<(), String> {
    for id in ["horizontal", "vertical", "diagonal", "translated"] {
        let path = tree
            .node_by_id(id)
            .ok_or_else(|| format!("missing stroke path {id}"))?;
        if path.abs_layer_bounding_box().is_none() {
            return Err(format!(
                "stroke path {id} must have a nonzero layer bounding box"
            ));
        }
    }
    Ok(())
}

fn assert_nontransparent(pixmap: &tiny_skia::Pixmap, x: u32, y: u32) -> Result<(), String> {
    let Some(pixel) = pixmap.pixel(x, y) else {
        return Err(format!("missing pixel at ({x}, {y})"));
    };
    if pixel.alpha() == 0 {
        return Err(format!("expected nontransparent pixel at ({x}, {y})"));
    }
    Ok(())
}

fn assert_region_nontransparent(
    pixmap: &tiny_skia::Pixmap,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
) -> Result<(), String> {
    for y in top..bottom {
        for x in left..right {
            if pixmap.pixel(x, y).is_some_and(|pixel| pixel.alpha() != 0) {
                return Ok(());
            }
        }
    }
    Err("expected text pixels in the lower region".to_string())
}
