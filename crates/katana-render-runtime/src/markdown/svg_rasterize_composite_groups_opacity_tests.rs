use super::super::super::{font, html_glyph::HtmlGlyphRasterizer};
use super::super::{AuthoredClipInfo, SvgCompositeRenderer, TextRasterizer};
use resvg::usvg;
use std::cell::Cell;
use tiny_skia::{Color, Pixmap, PixmapMut, Transform};

const WIDTH: u32 = 120;
const HEIGHT: u32 = 48;
const OPAQUE_OPACITY: f32 = 1.0;
const FADED_OPACITY: f32 = 0.8;
const TRANSLATE_X: u32 = 6;
const TRANSLATE_Y: u32 = 3;
const RGBA_CHANNELS: usize = 4;
const GROUP_SHAPES: &str = r#"<rect x="50" y="2" width="30" height="8" fill="black"/><rect x="65" y="4" width="30" height="8" fill="black"/>"#;
const GROUP_TEXT: &str =
    r#"<text x="4" y="32" font-family="Arial" font-size="24" fill="black">Gantt 10:30</text>"#;

struct ActualHtmlText<'a> {
    inner: HtmlGlyphRasterizer<'a>,
    calls: Cell<usize>,
    handled: Cell<bool>,
}

impl TextRasterizer for ActualHtmlText<'_> {
    fn render(
        &self,
        text: &usvg::Text,
        transform: Transform,
        pixmap: &mut PixmapMut<'_>,
    ) -> Result<bool, String> {
        self.calls.set(self.calls.get() + 1);
        let handled = self.inner.render(text, transform, pixmap)?;
        self.handled.set(handled);
        Ok(handled)
    }
}

#[test]
fn html_glyphs_and_overlapping_children_keep_group_opacity_once() -> Result<(), String> {
    let opaque = render_text(OPAQUE_OPACITY, None, None)?;
    let faded = render_text(FADED_OPACITY, None, None)?;
    assert!(opaque.calls > 0 && opaque.handled);
    assert!(faded.calls > 0 && faded.handled);
    assert_scaled_once_over_white(&opaque.pixmap, &faded.pixmap, FADED_OPACITY)
}

#[test]
fn nested_opacity_groups_apply_each_level_once() -> Result<(), String> {
    let opaque = render_text(OPAQUE_OPACITY, None, None)?;
    let nested = render_text(FADED_OPACITY, Some(FADED_OPACITY), None)?;
    assert!(nested.calls > 0 && nested.handled);
    assert_scaled_once_over_white(
        &opaque.pixmap,
        &nested.pixmap,
        FADED_OPACITY * FADED_OPACITY,
    )
}

#[test]
fn opacity_group_transform_is_applied_once() -> Result<(), String> {
    let original = render_text(FADED_OPACITY, None, None)?;
    let transform = format!("translate({TRANSLATE_X} {TRANSLATE_Y})");
    let translated = render_text(FADED_OPACITY, None, Some(&transform))?;
    assert!(translated.calls > 0 && translated.handled);
    assert_translated_over_white(&original.pixmap, &translated.pixmap)
}

#[test]
fn no_text_rasterizer_keeps_the_existing_resvg_group_route() -> Result<(), String> {
    let svg = opacity_svg(FADED_OPACITY, None, None, false, true);
    let options = font::html_rasterizer_options(&svg);
    let tree = usvg::Tree::from_str(&svg, &options).map_err(|error| error.to_string())?;
    let mut actual = Pixmap::new(WIDTH, HEIGHT).ok_or("pixmap allocation failed")?;
    let mut expected = Pixmap::new(WIDTH, HEIGHT).ok_or("pixmap allocation failed")?;
    actual.fill(Color::WHITE);
    expected.fill(Color::WHITE);
    SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut actual.as_mut(),
        &AuthoredClipInfo::default(),
        None,
    )?;
    resvg::render(&tree, Transform::identity(), &mut expected.as_mut());
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

#[test]
fn unsupported_opacity_filter_keeps_the_existing_resvg_route() -> Result<(), String> {
    let svg = opacity_svg(FADED_OPACITY, None, None, true, true);
    let actual = render_source(&svg)?;
    let expected = render_existing_svg(&svg)?;
    assert_eq!(actual.calls, 0);
    assert!(!actual.handled);
    assert_eq!(actual.pixmap.data(), expected.data());
    Ok(())
}

#[test]
fn textless_opacity_group_keeps_the_existing_resvg_route() -> Result<(), String> {
    let svg = opacity_svg(FADED_OPACITY, None, None, false, false);
    let actual = render_source(&svg)?;
    let expected = render_existing_svg(&svg)?;
    assert_eq!(actual.calls, 0);
    assert_eq!(actual.pixmap.data(), expected.data());
    Ok(())
}

struct RenderedText {
    pixmap: Pixmap,
    calls: usize,
    handled: bool,
}

fn render_text(
    opacity: f32,
    nested: Option<f32>,
    transform: Option<&str>,
) -> Result<RenderedText, String> {
    let svg = opacity_svg(opacity, nested, transform, false, true);
    render_source(&svg)
}

fn render_source(svg: &str) -> Result<RenderedText, String> {
    let options = font::html_rasterizer_options(svg);
    let tree = usvg::Tree::from_str(svg, &options).map_err(|error| error.to_string())?;
    let rasterizer = ActualHtmlText {
        inner: HtmlGlyphRasterizer::new(tree.fontdb()),
        calls: Cell::new(0),
        handled: Cell::new(false),
    };
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).ok_or("pixmap allocation failed")?;
    pixmap.fill(Color::WHITE);
    SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut pixmap.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    )?;
    Ok(RenderedText {
        pixmap,
        calls: rasterizer.calls.get(),
        handled: rasterizer.handled.get(),
    })
}

fn opacity_svg(
    opacity: f32,
    nested: Option<f32>,
    transform: Option<&str>,
    filtered: bool,
    include_text: bool,
) -> String {
    let transform = match transform {
        Some(value) => format!(" transform=\"{value}\""),
        None => String::new(),
    };
    let (filter_definition, filter_attribute) = filter_markup(filtered);
    let content = group_content(include_text);
    let content = match nested {
        Some(inner) => format!(
            "<g opacity=\"{opacity}\"{transform}{filter_attribute}><g opacity=\"{inner}\">{content}</g></g>"
        ),
        None => format!("<g opacity=\"{opacity}\"{transform}{filter_attribute}>{content}</g>"),
    };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}">{filter_definition}<rect width="100%" height="100%" fill="white"/>{content}</svg>"#
    )
}

fn filter_markup(enabled: bool) -> (&'static str, &'static str) {
    if enabled {
        (
            "<defs><filter id=\"blur\"><feGaussianBlur stdDeviation=\"0\"/></filter></defs>",
            " filter=\"url(#blur)\"",
        )
    } else {
        ("", "")
    }
}

fn group_content(include_text: bool) -> String {
    if include_text {
        format!("{GROUP_SHAPES}{GROUP_TEXT}")
    } else {
        GROUP_SHAPES.to_owned()
    }
}

fn assert_scaled_once_over_white(
    opaque: &Pixmap,
    faded: &Pixmap,
    opacity: f32,
) -> Result<(), String> {
    for (index, (full, actual)) in opaque.data().iter().zip(faded.data()).enumerate() {
        let background = f32::from(u8::MAX);
        let expected =
            (f32::from(*full) * opacity + background * (OPAQUE_OPACITY - opacity)).round() as u8;
        if actual.abs_diff(expected) > 1 {
            return Err(format!(
                "opacity group mismatch at channel {index}: expected {expected}, got {actual}"
            ));
        }
    }
    Ok(())
}

fn assert_translated_over_white(original: &Pixmap, translated: &Pixmap) -> Result<(), String> {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let dest_start = ((y * WIDTH + x) as usize) * RGBA_CHANNELS;
            assert_translated_pixel(original, translated, x, y, dest_start)?;
        }
    }
    Ok(())
}

fn assert_translated_pixel(
    original: &Pixmap,
    translated: &Pixmap,
    x: u32,
    y: u32,
    dest_start: usize,
) -> Result<(), String> {
    for channel in 0..RGBA_CHANNELS {
        let expected = translated_expected_channel(original, x, y, channel);
        if translated.data()[dest_start + channel].abs_diff(expected) > 1 {
            return Err("transformed opacity group was not translated once".into());
        }
    }
    Ok(())
}

fn translated_expected_channel(original: &Pixmap, x: u32, y: u32, channel: usize) -> u8 {
    match (x.checked_sub(TRANSLATE_X), y.checked_sub(TRANSLATE_Y)) {
        (Some(source_x), Some(source_y)) => {
            let source_start = ((source_y * WIDTH + source_x) as usize) * RGBA_CHANNELS;
            original.data()[source_start + channel]
        }
        _ => u8::MAX,
    }
}

fn render_existing_svg(svg: &str) -> Result<Pixmap, String> {
    let options = font::html_rasterizer_options(svg);
    let tree = usvg::Tree::from_str(svg, &options).map_err(|error| error.to_string())?;
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).ok_or("pixmap allocation failed")?;
    pixmap.fill(Color::WHITE);
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    Ok(pixmap)
}
