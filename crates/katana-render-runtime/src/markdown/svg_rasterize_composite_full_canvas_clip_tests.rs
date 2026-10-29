use super::render_if_covered;
use crate::markdown::svg_rasterize::composite::{
    AuthoredClipInfo, RenderContext, groups, layers::clip_mask,
};
use resvg::usvg::{Group, Node, Options, Tree};
use tiny_skia::{Color, Pixmap, Transform};

const SIZE: u32 = 32;
const DPR_SCALE: f32 = 2.0;
const DPR_SIZE: u32 = (SIZE as f32 * DPR_SCALE) as u32;
const DPR_SAMPLE_SCALE: u32 = 2;
const RGBA_CHANNELS: usize = 4;
const BASE_SAMPLE: u32 = 4;
const OVERLAY_SAMPLE: u32 = 12;
const BACKGROUND: [u8; 4] = [9, 17, 31, 255];
const BASE_COLOR: [u8; 4] = [220, 32, 20, 255];
const OVERLAY_COLOR: [u8; 4] = [10, 60, 230, 255];
const OVERLAPPING_ALPHA_CONTENT: &str = r##"<rect x="2" y="2" width="28" height="28" fill="#dc2014" fill-opacity="0.5"/><rect x="10" y="10" width="12" height="12" fill="#0a3ce6" fill-opacity="0.5"/>"##;
const FULL_RECT: &str =
    r#"<clipPath id="full"><rect x="-1" y="-1" width="34" height="34"/></clipPath>"#;
const PARTIAL_RECT: &str =
    r#"<clipPath id="full"><rect x="1" y="1" width="30" height="30"/></clipPath>"#;
const FRACTIONAL_DIAMOND: &str =
    r#"<clipPath id="full"><path d="M16.25 -1 L33 16.25 L16.25 33.5 L-1 16.25 Z"/></clipPath>"#;
const NESTED_HOLE: &str = r#"<clipPath id="hole"><path clip-rule="evenodd" d="M-1 -1 H33 V33 H-1 Z M10 10 H22 V22 H10 Z"/></clipPath><clipPath id="full" clip-path="url(#hole)"><rect x="-1" y="-1" width="34" height="34"/></clipPath>"#;

#[test]
fn full_canvas_rect_draws_directly_over_background_in_order_at_each_scale() -> Result<(), String> {
    assert_full_canvas_render(SIZE, Transform::identity(), BASE_SAMPLE, OVERLAY_SAMPLE)?;
    assert_full_canvas_render(
        DPR_SIZE,
        Transform::from_scale(DPR_SCALE, DPR_SCALE),
        BASE_SAMPLE * DPR_SAMPLE_SCALE,
        OVERLAY_SAMPLE * DPR_SAMPLE_SCALE,
    )
}

#[test]
fn overlapping_translucent_children_match_legacy_crop_merge() -> Result<(), String> {
    let actual = render_and_compare(
        FULL_RECT,
        OVERLAPPING_ALPHA_CONTENT,
        SIZE,
        Transform::identity(),
    )?;
    assert_ne!(pixel(&actual, BASE_SAMPLE, BASE_SAMPLE)?, BACKGROUND);
    assert_ne!(pixel(&actual, OVERLAY_SAMPLE, OVERLAY_SAMPLE)?, BASE_COLOR);
    Ok(())
}

fn assert_full_canvas_render(
    size: u32,
    transform: Transform,
    base_x: u32,
    overlay_x: u32,
) -> Result<(), String> {
    let actual = render_and_compare(FULL_RECT, CONTENT, size, transform)?;
    assert_eq!(pixel(&actual, 0, 0)?, BACKGROUND);
    assert_eq!(pixel(&actual, base_x, base_x)?, BASE_COLOR);
    assert_eq!(pixel(&actual, overlay_x, overlay_x)?, OVERLAY_COLOR);
    Ok(())
}

const CONTENT: &str = r##"<rect x="2" y="2" width="28" height="28" fill="#dc2014"/><rect x="10" y="10" width="12" height="12" fill="#0a3ce6"/>"##;

fn render_and_compare(
    clip_markup: &str,
    content: &str,
    size: u32,
    transform: Transform,
) -> Result<Pixmap, String> {
    let tree = tree_with_content(clip_markup, content)?;
    let group = layer(&tree)?;
    let clip = group.clip_path().ok_or("full rectangle clip missing")?;
    let mask = clip_mask(clip, transform, size, size)?;
    assert!(mask.data().iter().all(|coverage| *coverage == u8::MAX));
    let info = AuthoredClipInfo::default();
    let context = context(&info);
    let mut actual = target(size)?;
    let mut expected = target(size)?;
    assert!(render_if_covered(
        group,
        transform,
        &mut actual.as_mut(),
        &context
    )?);
    groups::render_clipped_with_extra(group, transform, &mut expected.as_mut(), &context, None)?;
    assert_eq!(actual.data(), expected.data());
    Ok(actual)
}

#[test]
fn partial_fractional_and_nested_hole_clips_refuse_without_writing() -> Result<(), String> {
    for (markup, candidate) in [
        (PARTIAL_RECT, false),
        (FRACTIONAL_DIAMOND, true),
        (NESTED_HOLE, true),
    ] {
        assert_refused_unchanged(markup, candidate)?;
    }
    Ok(())
}

fn assert_refused_unchanged(markup: &str, candidate: bool) -> Result<(), String> {
    let tree = tree(markup)?;
    let group = layer(&tree)?;
    let clip = group.clip_path().ok_or("candidate clip missing")?;
    assert_eq!(
        super::is_candidate(clip, Transform::identity(), SIZE, SIZE),
        candidate
    );
    if candidate {
        let mask = clip_mask(clip, Transform::identity(), SIZE, SIZE)?;
        assert!(mask.data().iter().any(|coverage| *coverage != u8::MAX));
    }
    let info = AuthoredClipInfo::default();
    let context = context(&info);
    let mut target = target(SIZE)?;
    let before = target.data().to_vec();
    assert!(!render_if_covered(
        group,
        Transform::identity(),
        &mut target.as_mut(),
        &context
    )?);
    assert_eq!(target.data(), before);
    Ok(())
}

fn tree(clip_markup: &str) -> Result<Tree, String> {
    tree_with_content(clip_markup, CONTENT)
}

fn tree_with_content(clip_markup: &str, content: &str) -> Result<Tree, String> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{SIZE}" height="{SIZE}"><defs>{clip_markup}</defs><g id="layer" clip-path="url(#full)">{content}</g></svg>"#
    );
    Tree::from_str(&svg, &Options::default()).map_err(|error| error.to_string())
}

fn layer(tree: &Tree) -> Result<&Group, String> {
    let Some(Node::Group(group)) = tree.node_by_id("layer") else {
        return Err("full canvas clip group missing".into());
    };
    Ok(group)
}

fn target(size: u32) -> Result<Pixmap, String> {
    let mut pixmap = Pixmap::new(size, size).ok_or("full canvas target unavailable")?;
    pixmap.fill(color(BACKGROUND));
    Ok(pixmap)
}

fn context<'a>(info: &'a AuthoredClipInfo) -> RenderContext<'a, 'a> {
    RenderContext {
        provenance: info,
        text: None,
        plan: None,
    }
}

fn color([red, green, blue, alpha]: [u8; RGBA_CHANNELS]) -> Color {
    Color::from_rgba8(red, green, blue, alpha)
}

fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> Result<[u8; RGBA_CHANNELS], String> {
    let offset = y
        .checked_mul(pixmap.width())
        .and_then(|row| row.checked_add(x))
        .and_then(|index| index.checked_mul(RGBA_CHANNELS as u32))
        .ok_or("pixel offset overflow")? as usize;
    let end = offset
        .checked_add(RGBA_CHANNELS)
        .ok_or("pixel range overflow")?;
    let channels = pixmap
        .data()
        .get(offset..end)
        .ok_or("pixel is outside target")?;
    channels
        .try_into()
        .map_err(|_| "pixel does not contain four channels".to_owned())
}
