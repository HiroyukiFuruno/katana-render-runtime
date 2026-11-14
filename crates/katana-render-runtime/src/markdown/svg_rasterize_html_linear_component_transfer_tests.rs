use super::super::{AuthoredClipInfo, SvgCompositeRenderer, TextRasterizer};
use resvg::usvg::Text;
use std::cell::Cell;
use tiny_skia::{Color, Paint, PixmapMut, Rect, Transform};

#[path = "svg_rasterize_html_linear_component_transfer_boundary_tests.rs"]
mod boundary_tests;

const SIZE: u32 = 40;
const CHANNEL_COUNT: usize = 4;
type RenderResult = (Vec<[u8; CHANNEL_COUNT]>, usize);
const RED_INDEX: usize = 0;
const GREEN_INDEX: usize = 1;
const BLUE_INDEX: usize = 2;
const RGB_CHANNEL_COUNT: usize = 3;
const ALPHA_INDEX: usize = RGB_CHANNEL_COUNT;
const LINEAR_SCALE: f32 = 1.2;
const CHANNEL_TOLERANCE: u8 = 1;
const HOOK_RED: u8 = 128;
const HOOK_GREEN: u8 = 64;
const HOOK_BLUE: u8 = 32;
const HOOK_ALPHA: u8 = 160;
const TRANSFORM_X: usize = 4;
const TRANSFORM_Y: usize = 5;
const FILTER_WIDTH: usize = 10;
const FILTER_HEIGHT: usize = 15;
const EXPECTED_LEFT: usize = TRANSFORM_X;
const EXPECTED_TOP: usize = TRANSFORM_Y;
const EXPECTED_RIGHT: usize = EXPECTED_LEFT + FILTER_WIDTH;
const EXPECTED_BOTTOM: usize = EXPECTED_TOP + FILTER_HEIGHT;
const IDENTITY_FUNCTIONS: &str = "<feFuncR type=\"linear\" slope=\"1\" intercept=\"0\"/><feFuncG type=\"linear\" slope=\"1\" intercept=\"0\"/><feFuncB type=\"linear\" slope=\"1\" intercept=\"0\"/><feFuncA type=\"identity\"/>";
const LINEAR_FUNCTIONS: &str = "<feFuncR type=\"linear\" slope=\"1.2\" intercept=\"0\"/><feFuncG type=\"linear\" slope=\"1.2\" intercept=\"0\"/><feFuncB type=\"linear\" slope=\"1.2\" intercept=\"0\"/><feFuncA type=\"identity\"/>";
const OFFSET_FUNCTIONS: &str = "<feFuncR type=\"linear\" slope=\"1\" intercept=\"0.1\"/><feFuncG type=\"linear\" slope=\"1\" intercept=\"0.2\"/><feFuncB type=\"linear\" slope=\"1\" intercept=\"0.3\"/><feFuncA type=\"identity\"/>";
const OFFSET_RED: f32 = 0.1;
const OFFSET_GREEN: f32 = 0.2;
const OFFSET_BLUE: f32 = 0.3;
const ALPHA_FUNCTIONS: &str = "<feFuncR type=\"linear\" slope=\"1\" intercept=\"0\"/><feFuncG type=\"linear\" slope=\"1\" intercept=\"0\"/><feFuncB type=\"linear\" slope=\"1\" intercept=\"0\"/><feFuncA type=\"linear\" slope=\"1\" intercept=\"0\"/>";
const TABLE_FUNCTIONS: &str = "<feFuncR type=\"table\" tableValues=\"0 1\"/><feFuncG type=\"linear\" slope=\"1\" intercept=\"0\"/><feFuncB type=\"linear\" slope=\"1\" intercept=\"0\"/><feFuncA type=\"identity\"/>";
const GAMMA_FUNCTIONS: &str = "<feFuncR type=\"gamma\" amplitude=\"1\" exponent=\"1\" offset=\"0\"/><feFuncG type=\"linear\" slope=\"1\" intercept=\"0\"/><feFuncB type=\"linear\" slope=\"1\" intercept=\"0\"/><feFuncA type=\"identity\"/>";
const FILTER_REGION: &str = " filterUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"40\" height=\"40\" color-interpolation-filters=\"sRGB\"";
const LINEAR_RGB_REGION: &str = " filterUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"40\" height=\"40\" color-interpolation-filters=\"linearRGB\"";
const TRANSFORM_FILTER_REGION: &str = " filterUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"10\" height=\"15\" color-interpolation-filters=\"sRGB\"";

fn component_transfer_svg(
    functions: &str,
    filter_attributes: &str,
    definitions: &str,
    group_attributes: &str,
    text_attributes: &str,
) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"40\"><defs>{definitions}<filter id=\"f\"{filter_attributes}><feComponentTransfer>{functions}</feComponentTransfer></filter></defs><g filter=\"url(#f)\"{group_attributes}><text id=\"label\" x=\"2\" y=\"20\" font-size=\"12\"{text_attributes}>text</text></g></svg>"
    )
}

fn plain_svg(text_attributes: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"40\"><text x=\"2\" y=\"20\" font-size=\"12\"{text_attributes}>text</text></svg>"
    )
}

struct Spy {
    calls: Cell<usize>,
}

impl TextRasterizer for Spy {
    fn render(
        &self,
        text: &Text,
        transform: Transform,
        pixmap: &mut PixmapMut<'_>,
    ) -> Result<bool, String> {
        self.calls.set(self.calls.get() + 1);
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(
            HOOK_RED, HOOK_GREEN, HOOK_BLUE, HOOK_ALPHA,
        ));
        let bounds = text
            .stroke_bounding_box()
            .to_non_zero_rect()
            .ok_or_else(|| "text hook fixture rectangle is invalid".to_owned())?;
        let rect = Rect::from_ltrb(bounds.left(), bounds.top(), bounds.right(), bounds.bottom())
            .ok_or_else(|| "text hook fixture rectangle is invalid".to_owned())?;
        pixmap.fill_rect(rect, &paint, transform, None);
        Ok(true)
    }
}

#[test]
fn component_transfer_uses_text_hook_and_identity_is_pixel_stable() -> Result<(), String> {
    let filtered = render_svg(&component_transfer_svg(
        IDENTITY_FUNCTIONS,
        FILTER_REGION,
        "",
        "",
        "",
    ))?;
    let plain = render_svg(&plain_svg(""))?;
    assert_eq!(filtered.0, plain.0);
    assert!(filtered.1 > 0);
    Ok(())
}

#[test]
fn component_transfer_changes_rgb() -> Result<(), String> {
    let text_attributes = " fill=\"#804020\" fill-opacity=\"0.5\"";
    let (linear, calls) = render_svg(&component_transfer_svg(
        LINEAR_FUNCTIONS,
        FILTER_REGION,
        "",
        "",
        text_attributes,
    ))?;
    let (plain, _) = render_svg(&plain_svg(text_attributes))?;
    assert!(calls > 0);
    assert_linear_rgb_transform(&linear, &plain);
    Ok(())
}

#[test]
fn component_transfer_applies_normalized_intercepts() -> Result<(), String> {
    let text_attributes = " fill=\"#804020\" fill-opacity=\"0.5\"";
    let (filtered, calls) = render_svg(&component_transfer_svg(
        OFFSET_FUNCTIONS,
        FILTER_REGION,
        "",
        "",
        text_attributes,
    ))?;
    let (source, _) = render_svg(&plain_svg(text_attributes))?;
    assert!(calls > 0);
    assert_linear_rgb_offset(&filtered, &source);
    Ok(())
}

#[test]
fn unsupported_component_transfer_falls_back() -> Result<(), String> {
    let unsupported = [
        (LINEAR_FUNCTIONS, LINEAR_RGB_REGION, "", ""),
        (ALPHA_FUNCTIONS, FILTER_REGION, "", ""),
        (TABLE_FUNCTIONS, FILTER_REGION, "", ""),
        (GAMMA_FUNCTIONS, FILTER_REGION, "", ""),
        (
            LINEAR_FUNCTIONS,
            FILTER_REGION,
            "<clipPath id=\"c\"><rect width=\"30\" height=\"30\"/></clipPath>",
            " clip-path=\"url(#c)\"",
        ),
    ];
    for (functions, filter_attributes, definitions, group_attributes) in unsupported {
        let svg = component_transfer_svg(
            functions,
            filter_attributes,
            definitions,
            group_attributes,
            "",
        );
        let (_, calls) = render_svg(&svg)?;
        assert_eq!(calls, 0);
    }
    Ok(())
}

#[test]
fn component_transfer_preserves_supported_transform_and_region_boundary() -> Result<(), String> {
    let svg = component_transfer_svg(
        LINEAR_FUNCTIONS,
        TRANSFORM_FILTER_REGION,
        "",
        " transform=\"translate(4 5)\"",
        "",
    );
    let expected = transformed_text_bounds(&svg)?;
    let (pixels, calls) = render_svg(&svg)?;
    assert!(calls > 0);
    let bounds = alpha_bounds(&pixels);
    assert!(bounds.2 > bounds.0 && bounds.3 > bounds.1);
    assert!(bounds.0 >= expected.0.max(EXPECTED_LEFT));
    assert!(bounds.1 >= expected.1.max(EXPECTED_TOP));
    assert!(bounds.2 <= expected.2.min(EXPECTED_RIGHT));
    assert!(bounds.3 <= expected.3.min(EXPECTED_BOTTOM));
    Ok(())
}

fn render_svg(svg: &str) -> Result<RenderResult, String> {
    let (tree, mut pixmap) =
        super::super::tests::parse_svg_pixmap_with_bundled_fonts(svg, SIZE, SIZE)?;
    let spy = Spy {
        calls: Cell::new(0),
    };
    SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut pixmap.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&spy),
    )?;
    let pixels = pixmap
        .data()
        .chunks_exact(CHANNEL_COUNT)
        .map(|pixel| {
            [
                pixel[RED_INDEX],
                pixel[GREEN_INDEX],
                pixel[BLUE_INDEX],
                pixel[ALPHA_INDEX],
            ]
        })
        .collect();
    Ok((pixels, spy.calls.get()))
}

fn transformed_text_bounds(svg: &str) -> Result<(usize, usize, usize, usize), String> {
    let (tree, _) = super::super::tests::parse_svg_pixmap_with_bundled_fonts(svg, SIZE, SIZE)?;
    let node = tree
        .node_by_id("label")
        .ok_or("text fixture node missing")?;
    let rect = node.abs_stroke_bounding_box();
    Ok((
        rect.left().floor() as usize,
        rect.top().floor() as usize,
        rect.right().ceil() as usize,
        rect.bottom().ceil() as usize,
    ))
}

fn assert_linear_rgb_transform(filtered: &[[u8; CHANNEL_COUNT]], source: &[[u8; CHANNEL_COUNT]]) {
    let mut changed = 0;
    for (filtered, source) in filtered.iter().zip(source) {
        assert_eq!(filtered[ALPHA_INDEX], source[ALPHA_INDEX]);
        for channel in 0..RGB_CHANNEL_COUNT {
            let expected = (f32::from(source[channel]) * LINEAR_SCALE)
                .round()
                .min(f32::from(u8::MAX)) as u8;
            assert!(filtered[channel].abs_diff(expected) <= CHANNEL_TOLERANCE);
            changed += usize::from(filtered[channel] != source[channel]);
        }
    }
    assert!(changed > 0);
}

fn assert_linear_rgb_offset(filtered: &[[u8; CHANNEL_COUNT]], source: &[[u8; CHANNEL_COUNT]]) {
    let offsets = [OFFSET_RED, OFFSET_GREEN, OFFSET_BLUE];
    for (filtered, source) in filtered.iter().zip(source) {
        assert_eq!(filtered[ALPHA_INDEX], source[ALPHA_INDEX]);
        for channel in 0..RGB_CHANNEL_COUNT {
            let expected = (f32::from(source[channel])
                + offsets[channel] * f32::from(source[ALPHA_INDEX]))
            .round()
            .min(f32::from(source[ALPHA_INDEX])) as u8;
            assert!(filtered[channel].abs_diff(expected) <= CHANNEL_TOLERANCE);
        }
    }
}

fn alpha_bounds(pixels: &[[u8; CHANNEL_COUNT]]) -> (usize, usize, usize, usize) {
    let mut bounds = (SIZE as usize, SIZE as usize, 0, 0);
    for (index, pixel) in pixels.iter().enumerate() {
        if pixel[ALPHA_INDEX] == 0 {
            continue;
        }
        let x = index % SIZE as usize;
        let y = index / SIZE as usize;
        bounds.0 = bounds.0.min(x);
        bounds.1 = bounds.1.min(y);
        bounds.2 = bounds.2.max(x + 1);
        bounds.3 = bounds.3.max(y + 1);
    }
    bounds
}
