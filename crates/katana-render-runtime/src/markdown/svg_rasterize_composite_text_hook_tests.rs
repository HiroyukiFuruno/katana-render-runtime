use super::{
    AuthoredClipInfo, FallbackTextRasterizer, SVG_CANVAS_SIZE, SvgCompositeRenderer,
    TextRasterizer, parse_svg_pixmap_with_bundled_fonts, render_existing_svg_with_bundled_fonts,
};
use std::cell::Cell;

pub(in super::super) const TEXT_RASTERIZER_FAILURE: &str = "unit text rasterizer failure";

#[derive(Default)]
pub(in super::super) struct FailingTextRasterizer {
    calls: Cell<usize>,
}

impl FailingTextRasterizer {
    pub(in super::super) fn calls(&self) -> usize {
        self.calls.get()
    }
}

impl TextRasterizer for FailingTextRasterizer {
    fn render(
        &self,
        _text: &resvg::usvg::Text,
        _transform: tiny_skia::Transform,
        _pixmap: &mut tiny_skia::PixmapMut<'_>,
    ) -> Result<bool, String> {
        self.calls.set(self.calls.get() + 1);
        Err(TEXT_RASTERIZER_FAILURE.to_owned())
    }
}

mod existing_resvg_fallback_tests {
    use super::super::assert_matches_existing_resvg;

    const SOURCE_OVER_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><clipPath id="window"><rect x="10" y="10" width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g clip-path="url(#window)"><rect width="40" height="40" fill="#e0e0e0"/></g></svg>"##;
    const ISOLATED_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><clipPath id="window"><rect x="10" y="10" width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g clip-path="url(#window)" style="isolation:isolate"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg>"##;
    const OPACITY_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><clipPath id="window"><rect x="10" y="10" width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g opacity="0.5" clip-path="url(#window)"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg>"##;
    const FILTER_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><filter id="blur"><feGaussianBlur stdDeviation="0"/></filter><clipPath id="window"><rect x="10" y="10" width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g filter="url(#blur)" clip-path="url(#window)"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg>"##;
    const MASK_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><mask id="window"><rect width="40" height="40" fill="white"/></mask><clipPath id="clip"><rect x="10" y="10" width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g mask="url(#window)" clip-path="url(#clip)"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg>"##;

    #[test]
    fn explicit_isolation_and_source_over_stay_on_existing_resvg_path() -> Result<(), String> {
        for svg in [SOURCE_OVER_SVG, ISOLATED_SVG] {
            assert_matches_existing_resvg(svg)?;
        }
        Ok(())
    }

    #[test]
    fn isolating_group_features_stay_on_existing_resvg_path() -> Result<(), String> {
        for svg in [OPACITY_SVG, FILTER_SVG, MASK_SVG] {
            assert_matches_existing_resvg(svg)?;
        }
        Ok(())
    }
}

const TEXT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><text x="2" y="20" font-size="12">text</text></svg>"##;
const TEXT_VIEWPORT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><svg x="5" y="5" width="30" height="30"><text x="2" y="20" font-size="12">text</text></svg></svg>"##;
const ROOT_TRANSFORM_TEXT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40" transform="translate(3 4)"><text x="2" y="20" font-size="12">text</text></svg>"##;
const FILTER_TEXT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><filter id="blur"><feGaussianBlur stdDeviation="0"/></filter></defs><g filter="url(#blur)"><text x="2" y="20" font-size="12">text</text></g></svg>"##;

struct CountingTextRasterizer {
    calls: Cell<usize>,
    first_transform: Cell<Option<(f32, f32)>>,
}

struct TextHookObservation {
    calls: usize,
    first_transform: Option<(f32, f32)>,
}

impl TextRasterizer for CountingTextRasterizer {
    fn render(
        &self,
        _text: &resvg::usvg::Text,
        transform: tiny_skia::Transform,
        _pixmap: &mut tiny_skia::PixmapMut<'_>,
    ) -> Result<bool, String> {
        self.calls.set(self.calls.get() + 1);
        self.first_transform.set(Some((transform.tx, transform.ty)));
        Ok(true)
    }
}

fn observe_text_hook(svg: &str) -> Result<TextHookObservation, String> {
    let (tree, mut pixmap) =
        parse_svg_pixmap_with_bundled_fonts(svg, SVG_CANVAS_SIZE, SVG_CANVAS_SIZE)?;
    let rasterizer = CountingTextRasterizer {
        calls: Cell::new(0),
        first_transform: Cell::new(None),
    };
    SvgCompositeRenderer::render_with_text(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    )?;
    Ok(TextHookObservation {
        calls: rasterizer.calls.get(),
        first_transform: rasterizer.first_transform.get(),
    })
}

#[test]
fn text_rasterizer_false_preserves_existing_resvg_pixels() -> Result<(), String> {
    let (tree, mut actual) =
        parse_svg_pixmap_with_bundled_fonts(TEXT_SVG, SVG_CANVAS_SIZE, SVG_CANVAS_SIZE)?;
    let expected = render_existing_svg_with_bundled_fonts(
        TEXT_SVG,
        SVG_CANVAS_SIZE,
        SVG_CANVAS_SIZE,
        tiny_skia::Transform::identity(),
    )?;
    SvgCompositeRenderer::render_with_text(
        &tree,
        tiny_skia::Transform::identity(),
        &mut actual.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&FallbackTextRasterizer),
    )?;
    assert!(expected.data().chunks_exact(4).any(|pixel| pixel[3] != 0));
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

#[test]
fn text_rasterizer_receives_generated_viewport_children() -> Result<(), String> {
    let observation = observe_text_hook(TEXT_VIEWPORT_SVG)?;
    assert_eq!(observation.calls, 1);
    Ok(())
}

#[test]
fn text_rasterizer_applies_root_transform_once() -> Result<(), String> {
    let observation = observe_text_hook(ROOT_TRANSFORM_TEXT_SVG)?;
    assert_eq!(observation.calls, 1);
    assert_eq!(observation.first_transform, Some((3.0, 4.0)));
    Ok(())
}

#[test]
fn text_rasterizer_skips_unsupported_filter_groups() -> Result<(), String> {
    let (tree, mut actual) =
        parse_svg_pixmap_with_bundled_fonts(FILTER_TEXT_SVG, SVG_CANVAS_SIZE, SVG_CANVAS_SIZE)?;
    let expected = render_existing_svg_with_bundled_fonts(
        FILTER_TEXT_SVG,
        SVG_CANVAS_SIZE,
        SVG_CANVAS_SIZE,
        tiny_skia::Transform::identity(),
    )?;
    let rasterizer = CountingTextRasterizer {
        calls: Cell::new(0),
        first_transform: Cell::new(None),
    };
    SvgCompositeRenderer::render_with_text(
        &tree,
        tiny_skia::Transform::identity(),
        &mut actual.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    )?;
    assert_eq!(rasterizer.calls.get(), 0);
    assert_eq!(actual.data(), expected.data());
    Ok(())
}
