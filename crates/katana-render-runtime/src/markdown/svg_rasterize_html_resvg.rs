use super::{GeneratedClipProvenance, RasterizedSvg, SvgRasterizeError, SvgRasterizeOps};
#[cfg(all(test, windows))]
use std::time::Instant;

pub(super) fn rasterize(
    svg_text: &str,
    scale: f32,
    provenance: &GeneratedClipProvenance,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    #[cfg(all(test, windows))]
    let started = diagnostic_start();
    let options = super::html_rasterizer_options(svg_text);
    #[cfg(all(test, windows))]
    let fonts_ready = started.map(|_| Instant::now());
    let compatible_svg = SvgRasterizeOps::preprocess_for_rasterizer(svg_text);
    #[cfg(all(test, windows))]
    let preprocessed = started.map(|_| Instant::now());
    let tree = SvgRasterizeOps::parse_compatible_tree(&compatible_svg, &options)?;
    #[cfg(all(test, windows))]
    let parsed = started.map(|_| Instant::now());
    let result =
        SvgRasterizeOps::render_html_tree_with_resvg_text(&tree, scale, &compatible_svg, provenance);
    #[cfg(all(test, windows))]
    if let [Some(start), Some(fonts), Some(pre), Some(tree)] =
        [started, fonts_ready, preprocessed, parsed]
    {
        report(svg_text.len(), [start, fonts, pre, tree, Instant::now()]);
    }
    result
}

#[cfg(all(test, windows))]
fn diagnostic_start() -> Option<Instant> {
    std::env::var_os("DEBUG")
        .is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case("true"))
        .then(Instant::now)
}

#[cfg(all(test, windows))]
fn report(svg_bytes: usize, [start, fonts, preprocessed, parsed, painted]: [Instant; 5]) {
    eprintln!(
        "[krr-html-trace] phase=svg_raster_stages svg_bytes={svg_bytes} font_prepare_us={} preprocess_us={} parse_shape_us={} viewport_paint_us={}",
        (fonts - start).as_micros(),
        (preprocessed - fonts).as_micros(),
        (parsed - preprocessed).as_micros(),
        (painted - parsed).as_micros(),
    );
}
