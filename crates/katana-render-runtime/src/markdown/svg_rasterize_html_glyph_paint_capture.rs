#[path = "svg_rasterize_html_glyph_paint_capture_draw.rs"]
mod draw;
#[path = "svg_rasterize_html_glyph_paint_capture_source.rs"]
mod source;
pub(super) use source::record_source;

use resvg::usvg::FontVariation;
use skia_safe::{Font, M44, Point, SurfaceProps};
use std::cell::RefCell;
use std::collections::HashMap;

const MAX_EVENTS: usize = 256;
const MAX_GLYPHS_PER_EVENT: usize = 512;

#[derive(Clone, Default)]
pub(in crate::markdown::svg_rasterize) struct Snapshot {
    pub(in crate::markdown::svg_rasterize) events: Vec<serde_json::Value>,
    pub(in crate::markdown::svg_rasterize) dropped: usize,
    instances: HashMap<u32, Vec<serde_json::Value>>,
    pending_source: Option<serde_json::Value>,
    pending_source_oversized: bool,
    pending_surface_props: Option<serde_json::Value>,
}

impl Snapshot {
    pub(in crate::markdown::svg_rasterize) fn report_value(&self) -> serde_json::Value {
        let status = if self.dropped > 0 {
            "overflow"
        } else if self.events.is_empty() {
            "empty"
        } else {
            "captured"
        };
        serde_json::json!({
            "schema": "krr-html-glyph-paint-capture-v1",
            "status": status,
            "maxEvents": MAX_EVENTS,
            "maxGlyphsPerEvent": MAX_GLYPHS_PER_EVENT,
            "capturedDrawCount": self.events.len(),
            "hasRows": !self.events.is_empty(),
            "rows": self.events,
            "dropped": self.dropped,
            "complete": self.dropped == 0,
        })
    }
}

thread_local! {
    static ACTIVE: RefCell<Option<Snapshot>> = const { RefCell::new(None) };
}

pub(in crate::markdown::svg_rasterize) struct Scope {
    previous: Option<Snapshot>,
}

impl Scope {
    pub(in crate::markdown::svg_rasterize) fn enter() -> Self {
        Self {
            previous: ACTIVE.with(|active| active.replace(Some(Snapshot::default()))),
        }
    }

    pub(in crate::markdown::svg_rasterize) fn snapshot(&self) -> Snapshot {
        ACTIVE.with(|active| {
            let active = active.borrow();
            let Some(snapshot) = active.as_ref() else {
                return Snapshot::default();
            };
            snapshot.clone()
        })
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|active| {
            *active.borrow_mut() = self.previous.take();
        });
    }
}

pub(super) fn record_draw(
    font: &Font,
    glyph_ids: &[u16],
    positions: &[Point],
    origin: Point,
    local_to_device: M44,
) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(snapshot) = active.as_mut() else {
            return;
        };
        append_draw(
            snapshot,
            font,
            glyph_ids,
            positions,
            origin,
            local_to_device,
        );
    });
}

pub(super) fn record_surface_props(props: SurfaceProps) {
    ACTIVE.with(|active| {
        if let Some(snapshot) = active.borrow_mut().as_mut() {
            snapshot.pending_surface_props = Some(draw::surface_props_value(props));
        }
    });
}

fn append_draw(
    snapshot: &mut Snapshot,
    font: &Font,
    glyph_ids: &[u16],
    positions: &[Point],
    origin: Point,
    local_to_device: M44,
) {
    let source = snapshot.pending_source.take();
    let surface_props = snapshot.pending_surface_props.take();
    let oversized = std::mem::take(&mut snapshot.pending_source_oversized);
    if snapshot.events.len() >= MAX_EVENTS || glyph_ids.len() > MAX_GLYPHS_PER_EVENT || oversized {
        snapshot.dropped = snapshot.dropped.saturating_add(1);
        return;
    }
    let typeface_id = font.typeface().unique_id();
    let coordinates = snapshot.instances.get(&typeface_id).map(Vec::as_slice);
    let mut event = draw::draw_event_value(
        font,
        glyph_ids,
        positions,
        origin,
        local_to_device,
        coordinates,
    );
    event["sourceSpan"] = source.unwrap_or(serde_json::Value::Null);
    event["surfaceProps"] = surface_props.unwrap_or(serde_json::Value::Null);
    snapshot.events.push(event);
}

pub(super) fn record_instance(typeface: &skia_safe::Typeface, applied: &[FontVariation]) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(snapshot) = active.as_mut() else {
            return;
        };
        if snapshot.instances.len() >= MAX_EVENTS
            && !snapshot.instances.contains_key(&typeface.unique_id())
        {
            snapshot.dropped = snapshot.dropped.saturating_add(1);
            return;
        }
        snapshot
            .instances
            .entry(typeface.unique_id())
            .or_insert_with(|| {
                applied
                    .iter()
                    .map(|axis| {
                        serde_json::json!({
                            "tag": String::from_utf8_lossy(&axis.tag), "value": axis.value,
                        })
                    })
                    .collect()
            });
    });
}

#[cfg(test)]
pub(super) const MAX_EVENTS_FOR_TEST: usize = MAX_EVENTS;

#[cfg(test)]
pub(super) const MAX_GLYPHS_PER_EVENT_FOR_TEST: usize = MAX_GLYPHS_PER_EVENT;
