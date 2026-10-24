use super::{RasterizedSvg, SvgRasterizeError};
use resvg::usvg::{Options, Size, Tree};

const WIDTH: u32 = 5;
const HEIGHT: u32 = 4;
const SCALE: f32 = 1.25;
const CHANNEL_COUNT: usize = 4;
const ALPHA_CHANNEL: usize = 3;
const MIN_EDGE_ALPHA: u8 = 190;
const MAX_EDGE_ALPHA: u8 = 194;
const DISPLAY_WIDTH: f32 = 4.0;
const DISPLAY_HEIGHT: f32 = 3.0;
type CandidateRenderer = fn(&str, Size, f32) -> Result<RasterizedSvg, SvgRasterizeError>;
type TreeCandidateRenderer = fn(&str, &Tree, f32) -> Result<RasterizedSvg, SvgRasterizeError>;

pub(super) fn render_selected(
    svg: &str,
    tree: &Tree,
    scale: f32,
) -> Option<Result<RasterizedSvg, SvgRasterizeError>> {
    let selected = std::env::var("KRR_SVG_CANDIDATE").ok()?;
    if let Some(render) = tree_candidate_renderer(&selected) {
        return Some(render(svg, tree, scale));
    }
    let render: CandidateRenderer = match selected.as_str() {
        #[cfg(all(target_os = "macos", feature = "svg-skia-evaluation"))]
        "skia" => super::skia_candidate::render,
        #[cfg(all(
            target_os = "macos",
            feature = "svg-skia-evaluation",
            feature = "svg-librsvg-evaluation"
        ))]
        "librsvg-skia" => super::librsvg_candidate::render_outlined_with_skia,
        #[cfg(feature = "svg-librsvg-evaluation")]
        "librsvg" => super::librsvg_candidate::render,
        #[cfg(feature = "svg-thorvg-evaluation")]
        "thorvg" => super::thorvg_candidate::render,
        _ => return None,
    };
    Some(render(svg, tree.size(), scale))
}

fn tree_candidate_renderer(selected: &str) -> Option<TreeCandidateRenderer> {
    match selected {
        #[cfg(all(target_os = "macos", feature = "svg-skia-evaluation"))]
        "skia-registered" => Some(super::skia_registered_candidate::render),
        #[cfg(all(
            target_os = "macos",
            feature = "svg-skia-evaluation",
            feature = "svg-librsvg-evaluation"
        ))]
        "librsvg-skia-registered" => Some(super::librsvg_candidate::render_registered),
        _ => None,
    }
}

pub(super) fn assert_candidate_contract(render: CandidateRenderer) -> Result<(), String> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="3"><rect width="4" height="3" fill="red"/></svg>"#;
    let tree = Tree::from_str(svg, &Options::default()).map_err(|error| error.to_string())?;
    let image = render(svg, tree.size(), SCALE).map_err(|error| error.to_string())?;
    if image.width != WIDTH || image.height != HEIGHT {
        return Err(format!(
            "unexpected dimensions: {}x{}",
            image.width, image.height
        ));
    }
    if image.display_width != DISPLAY_WIDTH || image.display_height != DISPLAY_HEIGHT {
        return Err(format!(
            "unexpected display dimensions: {}x{}",
            image.display_width, image.display_height
        ));
    }
    if image.rgba.len() != WIDTH as usize * HEIGHT as usize * CHANNEL_COUNT {
        return Err("rectangle dimensions or RGBA conversion differ".to_string());
    }
    assert_opaque_rows(&image)?;
    assert_fractional_row(&image)
}

fn assert_opaque_rows(image: &RasterizedSvg) -> Result<(), String> {
    for y in 0..HEIGHT - 1 {
        for x in 0..WIDTH {
            let pixel = pixel_at(image, x, y)?;
            if pixel != [u8::MAX, 0, 0, u8::MAX] {
                return Err(format!("opaque pixel ({x},{y}) differs: {pixel:?}"));
            }
        }
    }
    Ok(())
}

fn assert_fractional_row(image: &RasterizedSvg) -> Result<(), String> {
    let y = HEIGHT - 1;
    for x in 0..WIDTH {
        let pixel = pixel_at(image, x, y)?;
        let alpha = pixel[ALPHA_CHANNEL];
        if pixel[0] != alpha
            || pixel[1] != 0
            || pixel[2] != 0
            || !(MIN_EDGE_ALPHA..=MAX_EDGE_ALPHA).contains(&alpha)
        {
            return Err(format!("fractional pixel ({x},{y}) differs: {pixel:?}"));
        }
    }
    Ok(())
}

fn pixel_at(image: &RasterizedSvg, x: u32, y: u32) -> Result<[u8; CHANNEL_COUNT], String> {
    let start = (y as usize * WIDTH as usize + x as usize) * CHANNEL_COUNT;
    let end = start + CHANNEL_COUNT;
    let bytes = image
        .rgba
        .get(start..end)
        .ok_or_else(|| format!("pixel ({x},{y}) is outside RGBA buffer"))?;
    bytes
        .try_into()
        .map_err(|_| format!("pixel ({x},{y}) has invalid channel count"))
}
