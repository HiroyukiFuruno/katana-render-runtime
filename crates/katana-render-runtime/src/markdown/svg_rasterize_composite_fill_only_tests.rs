use super::super::{geometry, style};
use super::{
    AuthoredClipInfo, HtmlGlyphRasterizer, SIZE, SvgCompositeRenderer, allocate_pixmap, parse,
    svg_with_path,
};
use resvg::usvg;
use tiny_skia::{Pixmap, Transform};

const FRACTIONAL_FILL_PATH: &str = "M4.25 5.5H50.75V48.25H4.25Z";
const MATCHING_CHANNEL: u8 = 30;
const MATCHING_BACKDROP: [u8; 4] = [
    MATCHING_CHANNEL,
    MATCHING_CHANNEL,
    MATCHING_CHANNEL,
    u8::MAX,
];
const RGBA_CHANNELS: usize = 4;

#[test]
fn opaque_fill_only_matches_direct_skia_pixels() -> Result<(), String> {
    let svg = svg_with_path(
        FRACTIONAL_FILL_PATH,
        "id=\"fill-only-reference\" fill=\"#ff0000\"",
    );
    let (tree, mut actual) = parse(&svg)?;
    let Some(usvg::Node::Path(path)) = tree.node_by_id("fill-only-reference") else {
        return Err("missing fill-only reference path".to_owned());
    };
    let mut resvg_baseline = allocate_pixmap()?;
    resvg::render(&tree, Transform::identity(), &mut resvg_baseline.as_mut());
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut actual.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    )?;
    let expected = render_direct_skia_fill(path)?;
    assert_ne!(resvg_baseline.data(), expected.data());
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

#[test]
fn opaque_fill_matches_src_over_control_on_matching_backdrop() -> Result<(), String> {
    let svg = svg_with_path(
        FRACTIONAL_FILL_PATH,
        "id=\"fractional-fill\" fill=\"#1e1e1e\"",
    );
    let (tree, _) = parse(&svg)?;
    let Some(usvg::Node::Path(path)) = tree.node_by_id("fractional-fill") else {
        return Err("missing fractional fill path".to_owned());
    };
    let Some(Some(src_paint)) = style::fill(path.fill(), path.rendering_mode()) else {
        return Err("fractional fill is unsupported by Skia style".to_owned());
    };
    let mut src_over_paint = skia_safe::Paint::default();
    src_over_paint
        .set_color(skia_safe::Color::from_rgb(
            MATCHING_CHANNEL,
            MATCHING_CHANNEL,
            MATCHING_CHANNEL,
        ))
        .set_style(skia_safe::PaintStyle::Fill)
        .set_anti_alias(true)
        .set_blend_mode(skia_safe::BlendMode::SrcOver);
    let backdrop = solid_pixmap(MATCHING_BACKDROP)?;
    let src_over = render_skia_fill(path, &src_over_paint, &backdrop)?;
    let actual = render_skia_fill(path, &src_paint, &backdrop)?;
    assert_eq!(actual.data(), src_over.data());
    Ok(())
}

fn render_skia_fill(
    path: &usvg::Path,
    paint: &skia_safe::Paint,
    backdrop: &Pixmap,
) -> Result<Pixmap, String> {
    let converted = geometry::convert(path.data(), path.fill().map(|fill| fill.rule()))?;
    let mut rendered = backdrop.clone();
    let dimensions = (SIZE as i32, SIZE as i32);
    let info = skia_safe::ImageInfo::new(
        dimensions,
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Premul,
        None,
    );
    {
        let mut surface = skia_safe::surfaces::wrap_pixels(
            &info,
            rendered.data_mut(),
            info.min_row_bytes(),
            None,
        )
        .ok_or_else(|| "Skia fractional fill surface creation failed".to_owned())?;
        surface.canvas().draw_path(&converted, paint);
    }
    Ok(rendered)
}

fn solid_pixmap(color: [u8; RGBA_CHANNELS]) -> Result<Pixmap, String> {
    let mut pixmap = allocate_pixmap()?;
    for pixel in pixmap.data_mut().chunks_exact_mut(RGBA_CHANNELS) {
        pixel.copy_from_slice(&color);
    }
    Ok(pixmap)
}

fn render_direct_skia_fill(path: &usvg::Path) -> Result<Pixmap, String> {
    let Some(Some(paint)) = style::fill(path.fill(), path.rendering_mode()) else {
        return Err("fill-only reference is unsupported by Skia style".to_owned());
    };
    let transparent = allocate_pixmap()?;
    render_skia_fill(path, &paint, &transparent)
}
