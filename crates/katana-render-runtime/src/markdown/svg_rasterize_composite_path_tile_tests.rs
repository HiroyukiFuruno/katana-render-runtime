use super::{Rect, TILE_BORDER, TILE_SIZE, TILE_STEP, coverage, has_curve, tile_origins};
use resvg::usvg;
use tiny_skia::{Color, Pixmap, Transform};

const WIDTH: u32 = 512;
const HEIGHT: u32 = 128;
const CROP_X: u32 = 30;
const CHANNELS: usize = 4;
const TRANSLUCENT_BACKDROP: [u8; CHANNELS] = [32, 64, 96, 128];
const CURVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="128"><path d="M220 30 C240 5 280 5 300 30 C280 55 240 55 220 30 Z" fill="#ff0000" stroke="#0000ff" stroke-width="3"/></svg>"##;
const LINE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="128"><path d="M220 30 L300 30 L300 60 Z" fill="#ff0000"/></svg>"##;
const NESTED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="128"><svg x="200" y="10" width="120" height="100"><svg x="10" y="5" width="100" height="80"><path d="M10 30 C25 0 65 0 80 30 C65 60 25 60 10 30 Z" fill="#ff0000"/></svg></svg></svg>"##;

fn path(tree: &usvg::Tree) -> Result<&usvg::Path, String> {
    match tree.root().children().first() {
        Some(usvg::Node::Path(path)) => Ok(path),
        _ => Err("path missing".to_owned()),
    }
}

#[test]
fn nonlinear_selection_keeps_line_paths_on_the_existing_route() -> Result<(), String> {
    let options = usvg::Options::default();
    let curve = usvg::Tree::from_str(CURVE, &options).map_err(|e| e.to_string())?;
    let line = usvg::Tree::from_str(LINE, &options).map_err(|e| e.to_string())?;
    assert!(has_curve(path(&curve)?));
    assert!(!has_curve(path(&line)?));
    Ok(())
}

#[test]
fn tile_coverage_partitions_frame_without_gaps_or_overlap() {
    for width in [1, 255, 256, 257, 508, 512, 1272] {
        let frame = Rect {
            left: 0,
            top: 0,
            right: width,
            bottom: 1,
        };
        let mut x = 0;
        let mut previous_right = 0;
        loop {
            let tile = Rect {
                left: x,
                top: 0,
                right: (x + TILE_SIZE).min(width),
                bottom: 1,
            };
            let part = coverage(tile, frame);
            assert_eq!(part.left, previous_right);
            previous_right = part.right;
            if tile.right == width {
                break;
            }
            x += TILE_STEP;
        }
        assert_eq!(previous_right, width);
        assert_eq!(TILE_SIZE - TILE_STEP, TILE_BORDER * 2);
    }
}

#[test]
fn affected_range_selects_only_intersecting_coverage_tiles() {
    for (frame, start, end, expected) in [
        (1, 0, 1, (0, 0)),
        (255, 254, 255, (0, 0)),
        (256, 255, 256, (0, 0)),
        (257, 254, 255, (0, 0)),
        (257, 255, 256, (254, 254)),
        (257, 254, 256, (0, 254)),
        (511, 508, 509, (254, 254)),
        (511, 509, 510, (508, 508)),
        (1_000_000, 500_000, 500_001, (499_872, 499_872)),
        (1_000_000, 999_900, 999_901, (999_744, 999_744)),
    ] {
        assert_eq!(tile_origins(start, end, frame), expected);
    }
}

#[test]
fn nested_crop_origins_accumulate_in_frame_pixels() -> Result<(), String> {
    let context = super::super::super::TileContext {
        origin: (0, 0),
        frame: (WIDTH, HEIGHT),
    };
    let outer = tiny_skia::IntRect::from_xywh(10, 20, 400, 100).ok_or("invalid outer crop")?;
    let inner = tiny_skia::IntRect::from_xywh(30, 5, 300, 80).ok_or("invalid inner crop")?;
    let nested = context.cropped(outer)?.cropped(inner)?;
    assert_eq!(nested.origin, (40, 25));
    assert_eq!(nested.frame, (WIDTH, HEIGHT));
    Ok(())
}

struct FallbackText;

impl super::super::super::TextRasterizer for FallbackText {
    fn render(
        &self,
        _: &usvg::Text,
        _: Transform,
        _: &mut tiny_skia::PixmapMut<'_>,
    ) -> Result<bool, String> {
        Ok(false)
    }
}

#[test]
fn nested_generated_clips_keep_curve_inside_viewports() -> Result<(), String> {
    let tree =
        usvg::Tree::from_str(NESTED, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let mut pixels = Pixmap::new(WIDTH, HEIGHT).ok_or("allocation failed")?;
    super::super::super::SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut pixels.as_mut(),
        &super::super::super::AuthoredClipInfo::default(),
        Some(&FallbackText),
    )?;
    assert_eq!(
        pixels.pixel(250, 45).ok_or("missing inside pixel")?.red(),
        255
    );
    assert_eq!(
        pixels
            .pixel(195, 45)
            .ok_or("missing outside pixel")?
            .alpha(),
        0
    );
    Ok(())
}

#[test]
fn cropped_curve_keeps_frame_tile_origin_and_transparent_backdrop() -> Result<(), String> {
    let tree = usvg::Tree::from_str(CURVE, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let node = path(&tree)?;
    for scale in [1.0, 1.5] {
        let mut full = canvas(WIDTH)?;
        let mut cropped = canvas(WIDTH - CROP_X)?;
        let transform = Transform::from_scale(scale, scale);
        super::super::render_if_supported_at_origin(
            node,
            transform,
            &mut full.as_mut(),
            Some(frame_context(0)),
        )?;
        let local = Transform::from_translate(-(CROP_X as f32), 0.0).pre_concat(transform);
        super::super::render_if_supported_at_origin(
            node,
            local,
            &mut cropped.as_mut(),
            Some(frame_context(CROP_X as i32)),
        )?;
        assert_crop_equal(&full, &cropped);
    }
    Ok(())
}

fn frame_context(x: i32) -> super::super::super::TileContext {
    super::super::super::TileContext {
        origin: (x, 0),
        frame: (WIDTH, HEIGHT),
    }
}

fn canvas(width: u32) -> Result<Pixmap, String> {
    let mut pixmap = Pixmap::new(width, HEIGHT).ok_or("allocation failed")?;
    let [red, green, blue, alpha] = TRANSLUCENT_BACKDROP;
    pixmap.fill(Color::from_rgba8(red, green, blue, alpha));
    Ok(pixmap)
}

fn assert_crop_equal(full: &Pixmap, cropped: &Pixmap) {
    for y in 0..HEIGHT as usize {
        let start = (y * WIDTH as usize + CROP_X as usize) * CHANNELS;
        let length = (WIDTH - CROP_X) as usize * CHANNELS;
        let crop_start = y * length;
        assert_eq!(
            &full.data()[start..start + length],
            &cropped.data()[crop_start..crop_start + length]
        );
    }
}
