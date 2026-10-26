use skia_safe::{Font, M44, Point};
use std::cell::RefCell;

const MAX_EVENTS: usize = 256;
const MAX_GLYPHS_PER_EVENT: usize = 512;
const MATRIX_COMPONENT_COUNT: usize = 16;

#[derive(Clone, Default)]
pub(in crate::markdown::svg_rasterize) struct Snapshot {
    pub(in crate::markdown::svg_rasterize) events: Vec<serde_json::Value>,
    pub(in crate::markdown::svg_rasterize) dropped: usize,
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
        if snapshot.events.len() >= MAX_EVENTS || glyph_ids.len() > MAX_GLYPHS_PER_EVENT {
            snapshot.dropped = snapshot.dropped.saturating_add(1);
            return;
        }
        snapshot.events.push(draw_event_value(
            font,
            glyph_ids,
            positions,
            origin,
            local_to_device,
        ));
    });
}

fn draw_event_value(
    font: &Font,
    glyph_ids: &[u16],
    positions: &[Point],
    origin: Point,
    local_to_device: M44,
) -> serde_json::Value {
    let typeface = font.typeface();
    let mut matrix = [0.0; MATRIX_COMPONENT_COUNT];
    local_to_device.get_col_major(&mut matrix);
    serde_json::json!({
        "typefaceFamily": typeface.family_name(),
        "typefaceStyle": format!("{:?}", typeface.font_style()),
        "font": font_value(font),
        "glyphIds": glyph_ids,
        "positions": position_values(positions),
        "origin": [origin.x, origin.y],
        "localToDeviceColumnMajor": matrix,
    })
}

fn font_value(font: &Font) -> serde_json::Value {
    serde_json::json!({
        "size": font.size(),
        "scaleX": font.scale_x(),
        "skewX": font.skew_x(),
        "subpixel": font.is_subpixel(),
        "linearMetrics": font.is_linear_metrics(),
        "baselineSnap": font.is_baseline_snap(),
        "embeddedBitmaps": font.is_embedded_bitmaps(),
        "embolden": font.is_embolden(),
        "forceAutoHinting": font.is_force_auto_hinting(),
        "hinting": format!("{:?}", font.hinting()),
        "edging": format!("{:?}", font.edging()),
    })
}

fn position_values(positions: &[Point]) -> Vec<[f32; 2]> {
    positions
        .iter()
        .map(|position| [position.x, position.y])
        .collect()
}

#[cfg(test)]
pub(super) const MAX_EVENTS_FOR_TEST: usize = MAX_EVENTS;

#[cfg(test)]
pub(super) const MAX_GLYPHS_PER_EVENT_FOR_TEST: usize = MAX_GLYPHS_PER_EVENT;
