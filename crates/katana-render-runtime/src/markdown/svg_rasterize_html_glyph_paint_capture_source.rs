use super::super::super::PreparedSpan;
use super::{ACTIVE, MAX_EVENTS, MAX_GLYPHS_PER_EVENT};

pub(in crate::markdown::svg_rasterize) fn record_source(
    span: &PreparedSpan,
    raster_scale: f32,
    glyph_index: Option<usize>,
) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(snapshot) = active.as_mut() else {
            return;
        };
        snapshot.pending_source = None;
        snapshot.pending_source_oversized =
            span.glyphs.len() > MAX_GLYPHS_PER_EVENT || span.glyphs.len() != span.glyph_ids.len();
        if snapshot.pending_source_oversized || snapshot.events.len() >= MAX_EVENTS {
            return;
        }
        snapshot.pending_source = Some(source_value(span, raster_scale, glyph_index));
    });
}

fn source_value(
    span: &PreparedSpan,
    raster_scale: f32,
    glyph_index: Option<usize>,
) -> serde_json::Value {
    let chunk = span.chunk_alignment.map(|alignment| {
        serde_json::json!({
            "cssScale": alignment.css_scale,
            "paintCssSize": alignment.paint_css_size,
        })
    });
    serde_json::json!({
            "spanFontSize": span.font_size,
            "rasterScale": raster_scale,
            "drawGlyphIndex": glyph_index,
            "xAlignment": alignment_value(span),
            "chunkAlignment": chunk,
            "fallbackAlignment": span.fallback_alignment.as_ref().map(|alignment| serde_json::json!({
                "cssScale": alignment.css_scale,
                "paintCssSize": alignment.paint_css_size,
                "positionCount": alignment.css_positions.len(),
            })),
            "glyphs": glyph_values(span),
    })
}

fn alignment_value(span: &PreparedSpan) -> Option<serde_json::Value> {
    span.x_alignment.map(|alignment| {
        serde_json::json!({
            "anchorX": alignment.anchor_x,
            "originSnapCssScale": alignment.origin_snap_css_scale,
            "layout": alignment.layout.map(|layout| serde_json::json!({
                "width": layout.width,
                "anchor": format!("{:?}", layout.anchor),
                "cssScale": layout.css_scale,
            })),
        })
    })
}

fn glyph_values(span: &PreparedSpan) -> Vec<serde_json::Value> {
    let first = span.glyphs.first().map(|glyph| glyph.transform());
    span.glyphs.iter().zip(&span.glyph_ids).map(|(glyph, id)| {
        let transform = glyph.transform();
        serde_json::json!({
            "id": id,
            "fontSize": glyph.font_size(),
            "relativePosition": first.map(|origin| [transform.tx - origin.tx, transform.ty - origin.ty]),
            "transform": [transform.sx, transform.ky, transform.kx, transform.sy, transform.tx, transform.ty],
        })
    }).collect()
}
