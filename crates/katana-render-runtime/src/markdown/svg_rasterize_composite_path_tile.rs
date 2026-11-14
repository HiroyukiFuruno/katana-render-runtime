use super::{PreparedPath, draw_path, render_prepared_path, skia_matrix};
use resvg::usvg;
use skia_safe::{AlphaType, ColorType, ImageInfo, M44};
use tiny_skia::{PixmapMut, Transform};

#[path = "svg_rasterize_composite_path_tile_bounds.rs"]
mod bounds;
use bounds::{Rect, coverage, frame_bounds, path_bounds, target_bounds, tile_origins};

#[cfg(test)]
#[path = "svg_rasterize_composite_path_tile_tests.rs"]
mod tests;

const TILE_SIZE: i32 = 256;
const TILE_BORDER: i32 = 1;
const TILE_STEP: i32 = TILE_SIZE - 2 * TILE_BORDER;
const CHANNELS: usize = 4;

pub(super) fn has_curve(node: &usvg::Path) -> bool {
    node.data().segments().any(|segment| {
        matches!(
            segment,
            usvg::tiny_skia_path::PathSegment::QuadTo(..)
                | usvg::tiny_skia_path::PathSegment::CubicTo(..)
        )
    })
}

pub(super) fn render(
    target: &mut PixmapMut<'_>,
    node: &usvg::Path,
    path: &PreparedPath,
    transform: Transform,
    context: super::super::TileContext,
) -> Result<(), String> {
    let Some(bounds) = node.stroke_bounding_box().transform(transform) else {
        return render_prepared_path(target, path, transform, node.paint_order());
    };
    let frame = frame_bounds(context.frame)?;
    let area = target_bounds(target, context.origin)?;
    let path_bounds = path_bounds(bounds, context.origin);
    let Some(affected) = area.intersect(path_bounds).and_then(|r| r.intersect(frame)) else {
        return Ok(());
    };
    let state = RenderState {
        path,
        transform,
        order: node.paint_order(),
        frame,
        area,
        affected,
    };
    render_tiles(target, &state)
}

struct RenderState<'a> {
    path: &'a PreparedPath,
    transform: Transform,
    order: usvg::PaintOrder,
    frame: Rect,
    area: Rect,
    affected: Rect,
}

fn render_tiles(target: &mut PixmapMut<'_>, state: &RenderState<'_>) -> Result<(), String> {
    let mut scratch = vec![0; TILE_SIZE as usize * TILE_SIZE as usize * CHANNELS];
    let (first, last) = tile_origins(
        state.affected.top,
        state.affected.bottom,
        state.frame.bottom,
    );
    for y in (first..=last).step_by(TILE_STEP as usize) {
        render_tile_row(target, state, y, &mut scratch)?;
    }
    Ok(())
}

fn render_tile_row(
    target: &mut PixmapMut<'_>,
    state: &RenderState<'_>,
    y: i32,
    scratch: &mut [u8],
) -> Result<(), String> {
    let (first, last) = tile_origins(state.affected.left, state.affected.right, state.frame.right);
    for x in (first..=last).step_by(TILE_STEP as usize) {
        let content = Rect {
            left: x,
            top: y,
            right: x.saturating_add(TILE_SIZE).min(state.frame.right),
            bottom: y.saturating_add(TILE_SIZE).min(state.frame.bottom),
        };
        /* WHY: 両軸のtile_originsは影響範囲と交差するタイルだけを列挙する。 */
        let copy = coverage(content, state.frame).overlap_bounds(state.affected);
        render_tile(target, state, content, copy, scratch)?;
    }
    Ok(())
}

fn render_tile(
    target: &mut PixmapMut<'_>,
    state: &RenderState<'_>,
    tile: Rect,
    copy: Rect,
    scratch: &mut [u8],
) -> Result<(), String> {
    let width = (tile.right - tile.left) as usize;
    let height = (tile.bottom - tile.top) as usize;
    let bytes = &mut scratch[..width * height * CHANNELS];
    bytes.fill(0);
    copy_background(bytes, width, tile, target, state.area);
    draw_tile(bytes, width, height, tile, state)?;
    let target_width = target.width() as usize;
    copy_rows(
        target.data_mut(),
        target_width,
        state.area,
        bytes,
        width,
        tile,
        copy,
    );
    Ok(())
}

fn copy_background(bytes: &mut [u8], width: usize, tile: Rect, target: &PixmapMut<'_>, area: Rect) {
    if let Some(existing) = tile.intersect(area) {
        copy_rows(
            bytes,
            width,
            tile,
            target.as_ref().data(),
            target.width() as usize,
            area,
            existing,
        );
    }
}

fn draw_tile(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    tile: Rect,
    state: &RenderState<'_>,
) -> Result<(), String> {
    let info = ImageInfo::new(
        (width as i32, height as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut surface = skia_safe::surfaces::wrap_pixels(&info, bytes, info.min_row_bytes(), None)
        .ok_or("Skia tile surface creation failed")?;
    let canvas = surface.canvas();
    canvas.translate((
        (state.area.left - tile.left) as f32,
        (state.area.top - tile.top) as f32,
    ));
    canvas.concat_44(&M44::from(&skia_matrix(state.transform)));
    draw_path(canvas, state.path, state.order);
    Ok(())
}

fn copy_rows(
    dst: &mut [u8],
    dst_width: usize,
    dst_rect: Rect,
    src: &[u8],
    src_width: usize,
    src_rect: Rect,
    region: Rect,
) {
    let length = (region.right - region.left) as usize * CHANNELS;
    for y in region.top..region.bottom {
        let d = ((y - dst_rect.top) as usize * dst_width + (region.left - dst_rect.left) as usize)
            * CHANNELS;
        let s = ((y - src_rect.top) as usize * src_width + (region.left - src_rect.left) as usize)
            * CHANNELS;
        dst[d..d + length].copy_from_slice(&src[s..s + length]);
    }
}
