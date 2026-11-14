use super::tests::{FallbackTextRasterizer, parse_svg_pixmap_with_bundled_fonts};
use super::{AuthoredClipInfo, SvgCompositeRenderer};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const STROKE_PATHS: &str = r##"
<path id="horizontal" d="M8 16 H56" fill="none" stroke="#ff0000" stroke-width="2"/>
<path id="vertical" d="M16 8 V56" fill="none" stroke="#00ff00" stroke-width="2"/>
<path id="diagonal" d="M8 8 L56 56" fill="none" stroke="#0000ff" stroke-width="2"/>
<g transform="translate(3 4)"><path id="translated" d="M8 28 H56" fill="none" stroke="#ffffff" stroke-width="2"/></g>"##;
const STROKE_TEXT: &str = "<text x=\"2\" y=\"62\" font-size=\"10\">stroke</text>";

#[test]
fn axis_aligned_and_transformed_strokes_keep_the_existing_render_contract() -> Result<(), String> {
    let svg = stroke_svg(true);
    let (tree, mut actual) = parse_svg_pixmap_with_bundled_fonts(&svg, WIDTH, HEIGHT)?;
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
        None,
    )?;
    assert_eq!(actual.data(), expected.data());
    assert_nontransparent(&actual, 32, 16)?;
    assert_nontransparent(&actual, 16, 32)?;
    assert_nontransparent(&actual, 40, 40)?;
    assert_nontransparent(&actual, 35, 32)?;
    assert_region_nontransparent(&actual, 0, 57, WIDTH, HEIGHT)?;
    Ok(())
}

#[test]
fn html_path_adapter_matches_full_surface_reference_pixels() -> Result<(), String> {
    let svg = stroke_svg(true);
    let (tree, mut actual) = parse_svg_pixmap_with_bundled_fonts(&svg, WIDTH, HEIGHT)?;
    let (text_tree, mut expected) =
        parse_svg_pixmap_with_bundled_fonts(&stroke_svg(false), WIDTH, HEIGHT)?;
    let global_transform = tiny_skia::Transform::identity();
    render_full_surface_paths(&tree, global_transform, &mut expected.as_mut())?;
    resvg::render(&text_tree, global_transform, &mut expected.as_mut());
    SvgCompositeRenderer::render_with_text(
        &tree,
        global_transform,
        &mut actual.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&FallbackTextRasterizer),
    )?;
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

fn render_full_surface_paths(
    tree: &resvg::usvg::Tree,
    global_transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
) -> Result<(), String> {
    for id in ["horizontal", "vertical", "diagonal", "translated"] {
        let node = tree
            .node_by_id(id)
            .ok_or_else(|| format!("missing stroke path {id}"))?;
        let resvg::usvg::Node::Path(path) = node else {
            return Err(format!("stroke node {id} is not a path"));
        };
        let transform = global_transform.pre_concat(path.abs_transform());
        if !super::path::render_if_supported(path, transform, pixmap)? {
            return Err(format!("stroke path {id} is unsupported by the adapter"));
        }
    }
    Ok(())
}

fn stroke_svg(include_paths: bool) -> String {
    let paths = if include_paths { STROKE_PATHS } else { "" };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{WIDTH}\" height=\"{HEIGHT}\">{paths}{STROKE_TEXT}</svg>"
    )
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
