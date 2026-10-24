use super::try_render;
use resvg::usvg::{Options, Tree};
use tiny_skia::{Color, Pixmap, Transform};

const WIDTH: u32 = 32;
const HEIGHT: u32 = 32;
const RGBA_CHANNEL_COUNT: usize = 4;
const BOUND_COMPONENT_COUNT: usize = 4;
const BACKGROUND_RGBA: (u8, u8, u8, u8) = (32, 64, 96, 255);
const SCALE: f32 = 2.0;
const TRANSLATE_X: f32 = 3.0;
const TRANSLATE_Y: f32 = 4.0;
const EXPECTED_BOUNDS: [u32; BOUND_COMPONENT_COUNT] = [7, 10, 14, 19];
const SAMPLE_X: u32 = 4;
const SAMPLE_Y: u32 = 4;
const EXPECTED_ALPHA_PIXEL: [u8; RGBA_CHANNEL_COUNT] = [144, 32, 48, 255];
const MIN_BOUND_X: usize = 0;
const MIN_BOUND_Y: usize = 1;
const MAX_BOUND_X: usize = 2;
const MAX_BOUND_Y: usize = BOUND_COMPONENT_COUNT - 1;
const RED_INDEX: usize = 0;
const GREEN_INDEX: usize = 1;
const BLUE_INDEX: usize = 2;

#[test]
fn unsupported_paints_leave_the_pixmap_unchanged() -> Result<(), String> {
    for svg in [GRADIENT_FILL, GRADIENT_STROKE] {
        let tree = Tree::from_str(svg, &Options::default()).map_err(|error| error.to_string())?;
        let node = tree.node_by_id("shape").ok_or("missing candidate path")?;
        let mut pixmap = Pixmap::new(WIDTH, HEIGHT).ok_or("failed to allocate pixmap")?;
        let before = pixmap.data().to_vec();
        let handled = try_render(node, Transform::identity(), &mut pixmap.as_mut())?;
        if handled || pixmap.data() != before.as_slice() {
            return Err("unsupported paint was rendered or mutated the pixmap".to_string());
        }
    }
    Ok(())
}

#[test]
fn transformed_opaque_path_matches_analytical_ink_bounds() -> Result<(), String> {
    let transform = Transform::from_scale(SCALE, SCALE).post_translate(TRANSLATE_X, TRANSLATE_Y);
    let pixmap = render_path(OPAQUE_RECT, transform, None)?;
    let bounds = alpha_bounds(&pixmap).ok_or("transformed path produced no ink")?;
    if bounds != EXPECTED_BOUNDS {
        return Err(format!("unexpected transformed ink bounds: {bounds:?}"));
    }
    Ok(())
}

#[test]
fn translucent_fill_preserves_independent_premultiplied_alpha() -> Result<(), String> {
    let background = Color::from_rgba8(
        BACKGROUND_RGBA.0,
        BACKGROUND_RGBA.1,
        BACKGROUND_RGBA.2,
        BACKGROUND_RGBA.3,
    );
    let pixmap = render_path(TRANSLUCENT_RECT, Transform::identity(), Some(background))?;
    let pixel = pixmap
        .pixel(SAMPLE_X, SAMPLE_Y)
        .ok_or("missing translucent pixel")?;
    for (index, channel) in EXPECTED_ALPHA_PIXEL.into_iter().enumerate() {
        let actual = pixel_channel(pixel, index);
        if actual > channel.saturating_add(1) || channel > actual.saturating_add(1) {
            return Err(format!("channel {index} differs: {actual} != {channel}"));
        }
    }
    Ok(())
}

fn render_path(
    svg: &str,
    transform: Transform,
    background: Option<Color>,
) -> Result<Pixmap, String> {
    let tree = Tree::from_str(svg, &Options::default()).map_err(|error| error.to_string())?;
    let node = tree.node_by_id("shape").ok_or("missing candidate path")?;
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).ok_or("failed to allocate pixmap")?;
    if let Some(color) = background {
        pixmap.fill(color);
    }
    if !try_render(node, transform, &mut pixmap.as_mut())? {
        return Err("supported path was rejected".to_string());
    }
    Ok(pixmap)
}

fn pixel_channel(pixel: tiny_skia::PremultipliedColorU8, index: usize) -> u8 {
    match index {
        RED_INDEX => pixel.red(),
        GREEN_INDEX => pixel.green(),
        BLUE_INDEX => pixel.blue(),
        _ => pixel.alpha(),
    }
}

fn alpha_bounds(pixmap: &Pixmap) -> Option<[u32; BOUND_COMPONENT_COUNT]> {
    let mut bounds = [WIDTH, HEIGHT, 0, 0];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if pixmap.pixel(x, y).is_some_and(|pixel| pixel.alpha() != 0) {
                bounds[MIN_BOUND_X] = bounds[MIN_BOUND_X].min(x);
                bounds[MIN_BOUND_Y] = bounds[MIN_BOUND_Y].min(y);
                bounds[MAX_BOUND_X] = bounds[MAX_BOUND_X].max(x);
                bounds[MAX_BOUND_Y] = bounds[MAX_BOUND_Y].max(y);
            }
        }
    }
    (bounds[MIN_BOUND_X] < WIDTH).then_some(bounds)
}

const GRADIENT_FILL: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><path id="shape" d="M2 2H12V12H2Z" fill="url(#g)"/></svg>"#;
const GRADIENT_STROKE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><path id="shape" d="M2 2H12V12H2Z" fill="red" stroke="url(#g)" stroke-width="2"/></svg>"#;
const OPAQUE_RECT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><path id="shape" d="M2 3H6V8H2Z" fill="white"/></svg>"#;
const TRANSLUCENT_RECT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><path id="shape" d="M2 2H12V12H2Z" fill="red" fill-opacity="0.5"/></svg>"#;
