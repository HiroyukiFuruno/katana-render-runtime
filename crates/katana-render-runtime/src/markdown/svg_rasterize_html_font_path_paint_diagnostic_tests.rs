use super::super::with_actual_route;
use super::{Transform, usvg};
use tiny_skia::Pixmap;

const SIZE: u32 = 64;
const CURVE_BOUNDS: [f32; super::super::CROP_RECT_COMPONENTS] = [10.0, 4.0, 30.0, 6.0];
const CURVE_SEGMENT_COUNT: usize = 3;
const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><defs><clipPath id="a"><rect x="2" y="2" width="50" height="50"/></clipPath><clipPath id="b"><rect x="5" y="5" width="35" height="35"/></clipPath><linearGradient id="g"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><g clip-path="url(#a)"><g clip-path="url(#b)"><path d="M10 10C20 4 30 4 40 10Z" fill="red" stroke="black" stroke-width="2"/><path d="M10 10C20 4 30 4 40 10Z" fill="red" stroke="black" stroke-width="2"/><path d="M5 30H20V40H5Z" fill="url(#g)"/></g></g></svg>"##;

fn render() -> Result<Pixmap, String> {
    render_test_svg(SVG, SIZE)
}

#[test]
fn records_real_paint_and_nested_crops_without_changing_pixels() -> Result<(), String> {
    let ordinary = render()?;
    let (audited, capture) = with_actual_route(render);
    assert_eq!(ordinary.data(), audited?.data());
    assert_eq!(capture.path_paints_dropped, 0);
    assert_eq!(capture.path_paints.len(), 2);
    assert_geometry_details(&capture.path_paints[0]);
    assert_paint_records(&capture.path_paints);
    assert_eq!(capture.path_fallback_hits, 1);
    assert!(capture.crop_stack.is_empty());
    Ok(())
}

fn assert_paint_records(records: &[serde_json::Value]) {
    assert_eq!(records[0]["geometrySha256"], records[1]["geometrySha256"]);
    assert_ne!(records[0]["sequence"], records[1]["sequence"]);
    assert_eq!(records[0]["crops"].as_array().map(Vec::len), Some(2));
    assert_eq!(records[0]["stroke"]["width"], 2.0);
    assert_eq!(records[0]["stroke"]["antiAlias"], true);
    assert_eq!(records[0]["paintOrder"], "FillAndStroke");
}

fn assert_geometry_details(record: &serde_json::Value) {
    assert_eq!(record["nodeId"], "");
    assert_eq!(record["localBounds"], serde_json::json!(CURVE_BOUNDS));
    assert_eq!(record["segmentCount"], CURVE_SEGMENT_COUNT);
}

#[test]
fn nested_crop_scopes_close_when_operation_returns_error() -> Result<(), String> {
    let crop = tiny_skia::IntRect::from_xywh(0, 0, SIZE, SIZE).ok_or("invalid crop")?;
    let (result, capture) = with_actual_route(|| -> Result<(), &str> {
        let _outer = super::SvgRasterizeOps::enter_path_crop(crop);
        let _inner = super::SvgRasterizeOps::enter_path_crop(crop);
        Err("intentional error")
    });
    assert!(result.is_err());
    assert!(capture.crop_stack.is_empty());
    assert_eq!(capture.crop_overflow, 0);
    assert!(super::SvgRasterizeOps::enter_path_crop(crop).is_none());
    Ok(())
}

pub(super) fn render_test_svg(input: &str, size: u32) -> Result<tiny_skia::Pixmap, String> {
    use crate::markdown::svg_rasterize::{
        composite::SvgCompositeRenderer, html_glyph::HtmlGlyphRasterizer,
        provenance::AuthoredClipInfo,
    };
    let tree = usvg::Tree::from_str(input, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size).ok_or("pixmap allocation failed")?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut pixmap.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    )?;
    Ok(pixmap)
}
