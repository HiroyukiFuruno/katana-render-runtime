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
#[path = "svg_rasterize_html_font_path_paint_boundary_tests.rs"]
mod boundary_tests;

#[cfg(test)]
#[path = "svg_rasterize_html_font_path_paint_diagnostic_tests.rs"]
mod tests;
