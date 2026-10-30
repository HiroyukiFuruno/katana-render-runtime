use tiny_skia::PixmapMut;

const AA_MARGIN: f32 = 2.0;

#[derive(Clone, Copy)]
pub(super) struct Rect {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) right: i32,
    pub(super) bottom: i32,
}

impl Rect {
    pub(super) fn intersect(self, other: Self) -> Option<Self> {
        let result = Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (result.left < result.right && result.top < result.bottom).then_some(result)
    }
}

pub(super) fn frame_bounds(size: (u32, u32)) -> Result<Rect, String> {
    Ok(Rect {
        left: 0,
        top: 0,
        right: i32::try_from(size.0).map_err(|e| e.to_string())?,
        bottom: i32::try_from(size.1).map_err(|e| e.to_string())?,
    })
}

pub(super) fn target_bounds(target: &PixmapMut<'_>, origin: (i32, i32)) -> Result<Rect, String> {
    let (left, top) = origin;
    let width = i32::try_from(target.width()).map_err(|e| e.to_string())?;
    let height = i32::try_from(target.height()).map_err(|e| e.to_string())?;
    Ok(Rect {
        left,
        top,
        right: left.checked_add(width).ok_or("tile bounds overflow")?,
        bottom: top.checked_add(height).ok_or("tile bounds overflow")?,
    })
}

pub(super) fn path_bounds(bounds: tiny_skia::Rect, origin: (i32, i32)) -> Rect {
    let (ox, oy) = origin;
    Rect {
        left: ((bounds.left() - AA_MARGIN).floor() as i32).saturating_add(ox),
        top: ((bounds.top() - AA_MARGIN).floor() as i32).saturating_add(oy),
        right: ((bounds.right() + AA_MARGIN).ceil() as i32).saturating_add(ox),
        bottom: ((bounds.bottom() + AA_MARGIN).ceil() as i32).saturating_add(oy),
    }
}

pub(super) fn coverage(tile: Rect, frame: Rect) -> Rect {
    use super::TILE_BORDER;
    Rect {
        left: tile.left + i32::from(tile.left != 0) * TILE_BORDER,
        top: tile.top + i32::from(tile.top != 0) * TILE_BORDER,
        right: tile.right - i32::from(tile.right != frame.right) * TILE_BORDER,
        bottom: tile.bottom - i32::from(tile.bottom != frame.bottom) * TILE_BORDER,
    }
}

pub(super) fn tile_origins(start: i32, end: i32, frame_size: i32) -> (i32, i32) {
    use super::{TILE_BORDER, TILE_SIZE, TILE_STEP};
    let last_tile = if frame_size <= TILE_SIZE {
        0
    } else {
        (frame_size - TILE_SIZE - 1) / TILE_STEP + 1
    };
    let tile_for_pixel =
        |pixel: i32| (pixel.saturating_sub(TILE_BORDER) / TILE_STEP).min(last_tile) * TILE_STEP;
    (tile_for_pixel(start), tile_for_pixel(end - 1))
}
