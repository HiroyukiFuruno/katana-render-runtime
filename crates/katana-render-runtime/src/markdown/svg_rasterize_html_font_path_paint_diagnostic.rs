use super::{ACTIVE, MAX_CAPTURED_RECORDS, PASSIVE_AUDIT};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use resvg::usvg;
use sha2::{Digest, Sha256};
use skia_safe::Paint;
use tiny_skia::{IntRect, Transform};

pub(in crate::markdown::svg_rasterize) struct CropScope;

impl Drop for CropScope {
    fn drop(&mut self) {
        ACTIVE.with(|active| {
            if let Some(capture) = active.borrow_mut().as_mut() {
                if capture.crop_overflow > 0 {
                    capture.crop_overflow -= 1;
                } else {
                    capture.crop_stack.pop();
                }
            }
        });
    }
}

impl SvgRasterizeOps {
    pub(in crate::markdown::svg_rasterize) fn enter_path_crop(crop: IntRect) -> Option<CropScope> {
        if !PASSIVE_AUDIT.with(std::cell::Cell::get) {
            return None;
        }
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            let capture = active.as_mut()?;
            if capture.crop_stack.len() == MAX_CAPTURED_RECORDS || capture.crop_overflow > 0 {
                capture.crop_overflow += 1;
            } else {
                capture.crop_stack.push([
                    i64::from(crop.x()),
                    i64::from(crop.y()),
                    i64::from(crop.width()),
                    i64::from(crop.height()),
                ]);
            }
            Some(CropScope)
        })
    }

    pub(in crate::markdown::svg_rasterize) fn record_path_paint(
        node: &usvg::Path,
        transform: Transform,
        dimensions: (u32, u32),
        order: usvg::PaintOrder,
        fill: Option<&Paint>,
        stroke: Option<&Paint>,
    ) {
        if !PASSIVE_AUDIT.with(std::cell::Cell::get) {
            return;
        }
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            let Some(capture) = active.as_mut() else {
                return;
            };
            if capture.path_paints.len() >= MAX_CAPTURED_RECORDS || capture.crop_overflow > 0 {
                capture.path_paints_dropped += 1;
                return;
            }
            let mut record = paint_record(node, transform, dimensions, order, fill, stroke);
            record["sequence"] = serde_json::json!(capture.path_paints.len());
            record["crops"] = serde_json::json!(capture.crop_stack);
            capture.path_paints.push(record);
        });
    }
}

fn paint_record(
    node: &usvg::Path,
    transform: Transform,
    dimensions: (u32, u32),
    order: usvg::PaintOrder,
    fill: Option<&Paint>,
    stroke: Option<&Paint>,
) -> serde_json::Value {
    serde_json::json!({
        "geometrySha256": geometry_signature(node),
        "nodeId": node.id(),
        "localBounds": local_bounds(node),
        "segmentCount": node.data().segments().count(),
        "matrix": [transform.sx, transform.kx, transform.tx, transform.ky, transform.sy, transform.ty],
        "targetSize": [dimensions.0, dimensions.1],
        "paintOrder": format!("{order:?}"),
        "fill": paint(fill),
        "stroke": paint(stroke),
    })
}

fn local_bounds(node: &usvg::Path) -> [f32; super::CROP_RECT_COMPONENTS] {
    let bounds = node.data().bounds();
    [bounds.x(), bounds.y(), bounds.width(), bounds.height()]
}

fn geometry_signature(node: &usvg::Path) -> String {
    let mut hash = Sha256::new();
    hash.update(format!("{:?}", node.data().verbs()).as_bytes());
    for point in node.data().points() {
        hash.update(point.x.to_bits().to_le_bytes());
        hash.update(point.y.to_bits().to_le_bytes());
    }
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn paint(value: Option<&Paint>) -> serde_json::Value {
    value.map_or(serde_json::Value::Null, |paint| {
        serde_json::json!({
            "antiAlias": paint.is_anti_alias(),
            "width": paint.stroke_width(),
            "cap": format!("{:?}", paint.stroke_cap()),
            "join": format!("{:?}", paint.stroke_join()),
        })
    })
}

#[cfg(test)]
fn render_test_svg(input: &str, size: u32) -> Result<tiny_skia::Pixmap, String> {
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

#[cfg(test)]
#[path = "svg_rasterize_html_font_path_paint_boundary_tests.rs"]
mod boundary_tests;

#[cfg(test)]
mod tests {
    use super::super::with_actual_route;
    use tiny_skia::Pixmap;

    const SIZE: u32 = 64;
    const CURVE_BOUNDS: [f32; super::super::CROP_RECT_COMPONENTS] = [10.0, 4.0, 30.0, 6.0];
    const CURVE_SEGMENT_COUNT: usize = 3;
    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><defs><clipPath id="a"><rect x="2" y="2" width="50" height="50"/></clipPath><clipPath id="b"><rect x="5" y="5" width="35" height="35"/></clipPath><linearGradient id="g"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><g clip-path="url(#a)"><g clip-path="url(#b)"><path d="M10 10C20 4 30 4 40 10Z" fill="red" stroke="black" stroke-width="2"/><path d="M10 10C20 4 30 4 40 10Z" fill="red" stroke="black" stroke-width="2"/><path d="M5 30H20V40H5Z" fill="url(#g)"/></g></g></svg>"##;

    fn render() -> Result<Pixmap, String> {
        super::render_test_svg(SVG, SIZE)
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
}
