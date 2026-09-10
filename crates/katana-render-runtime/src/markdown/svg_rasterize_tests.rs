use super::font::{
    bundled_font_db, current_process_rss_kib, html_font_db_for_markup, html_font_db_for_text,
    html_font_memory_snapshot, html_rasterizer_options, rasterizer_options,
    rasterizer_options_with_font_db,
};
use super::{RasterTarget, SvgRasterizeOps, effective_scale, parse_light_dark_function};
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::mermaid_renderer::MermaidRenderOps;
use crate::markdown::runtime_assets::RuntimeAsset;
use crate::markdown::{DiagramBlock, DiagramKind, DiagramResult};
use resvg::usvg;

const RGBA_CHANNELS: usize = 4;

#[test]
fn rasterize_svg_returns_pixels_for_simple_svg() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#fff"/></svg>"##;
    let image = SvgRasterizeOps::rasterize_svg(svg, 1.0);

    assert!(image.as_ref().is_ok_and(|it| it.width == 10));
    assert!(image.as_ref().is_ok_and(|it| it.height == 10));
    assert!(image.as_ref().is_ok_and(|it| !it.rgba.is_empty()));
}

#[test]
fn rasterize_svg_renders_text_with_the_bundled_font() -> Result<(), String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="96" height="32"><rect width="96" height="32" fill="#fff"/><text x="4" y="24" font-family="Noto Sans, sans-serif" font-size="20" fill="#14532d">KRR</text></svg>"##;
    let options = rasterizer_options_with_font_db(bundled_font_db());
    let tree = usvg::Tree::from_str(svg, &options).map_err(|error| error.to_string())?;
    let image = RasterTarget::new(tree.size(), 1.0)
        .render(&tree)
        .map_err(|error| error.to_string())?;

    assert!(
        image
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| *pixel != [255, 255, 255, 255])
    );
    Ok(())
}

#[test]
fn rasterize_svg_renders_distinct_japanese_glyphs_with_system_font_fallback() -> Result<(), String>
{
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="48"><rect width="240" height="48" fill="#fff"/><g font-family="Noto Sans JP, Noto Sans, sans-serif" font-size="32" fill="#14532d"><text x="8" y="36">日</text><text x="56" y="36">本</text><text x="104" y="36">語</text><text x="152" y="36">入</text><text x="200" y="36">力</text></g></svg>"##;
    let image = SvgRasterizeOps::rasterize_html_svg(svg, 1.0).map_err(|error| error.to_string())?;

    let painted_pixels = image
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| **pixel != [255, 255, 255, 255])
        .count();
    assert!(painted_pixels > 500, "painted pixels: {painted_pixels}");

    let distinct_cells = distinct_cell_count(&image.rgba, image.height, 240, 48, 5);
    assert!(
        distinct_cells >= 4,
        "Japanese glyph cells collapsed to repeated missing-glyph boxes"
    );
    Ok(())
}

#[test]
fn html_text_measurement_uses_the_same_shaping_as_rasterization() -> Result<(), String> {
    let family = "Noto Sans";
    let prefix = "LibreChat fork to MCP Hub to Code Sandbox in three layers";
    let full = "LibreChat fork to MCP Hub to Code Sandbox in three layers architecture";
    let prefix_width = SvgRasterizeOps::measure_html_text(
        prefix,
        family,
        42.842,
        700,
        false,
        0.42842,
        Some(r#""palt" 1"#),
    )
    .map_err(|error| error.to_string())?;
    let full_width = SvgRasterizeOps::measure_html_text(
        full,
        family,
        42.842,
        700,
        false,
        0.42842,
        Some(r#""palt" 1"#),
    )
    .map_err(|error| error.to_string())?;

    assert!(prefix_width <= 1230.0, "prefix width: {prefix_width}");
    assert!(full_width > 1230.0, "full width: {full_width}");
    Ok(())
}

fn distinct_cell_count(
    image: &[u8],
    image_height: u32,
    image_width: usize,
    cell_width: usize,
    cell_count: usize,
) -> usize {
    (0..cell_count)
        .map(|cell| {
            let mut pixels = Vec::with_capacity(cell_width * image_height as usize * RGBA_CHANNELS);
            for row in 0..image_height as usize {
                let start = (row * image_width + cell * cell_width) * RGBA_CHANNELS;
                pixels.extend_from_slice(&image[start..start + cell_width * RGBA_CHANNELS]);
            }
            pixels
        })
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

#[test]
fn rasterizer_prefers_the_bundled_noto_sans_before_system_fonts() -> Result<(), String> {
    let database = html_font_db_for_text("Noto Sans", "KRR");
    let query = usvg::fontdb::Query {
        families: &[usvg::fontdb::Family::Name("Noto Sans")],
        weight: usvg::fontdb::Weight::NORMAL,
        stretch: usvg::fontdb::Stretch::Normal,
        style: usvg::fontdb::Style::Normal,
    };
    let id = database
        .query(&query)
        .ok_or_else(|| "bundled Noto Sans was not found".to_string())?;
    let face = database
        .face(id)
        .ok_or_else(|| "selected Noto Sans face was not found".to_string())?;

    assert!(matches!(face.source, usvg::fontdb::Source::Binary(_)));
    Ok(())
}

#[test]
fn public_and_html_rasterizers_use_separate_font_databases() {
    let public = rasterizer_options();
    let html = html_rasterizer_options("<svg/>");

    assert!(std::sync::Arc::ptr_eq(&public.fontdb, &bundled_font_db()));
    assert!(std::sync::Arc::ptr_eq(
        &html.fontdb,
        &html_font_db_for_markup("<svg/>")
    ));
    assert!(!std::sync::Arc::ptr_eq(&public.fontdb, &html.fontdb));
}

#[test]
fn cold_html_font_loading_keeps_system_font_bytes_out_of_the_process_cache() -> Result<(), String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="192" height="48"><rect width="192" height="48" fill="#fff"/><text x="4" y="36" font-family="Helvetica Neue, Arial, sans-serif" font-size="28" fill="#14532d">日本🙂</text></svg>"##;
    let rss_before = current_process_rss_kib();
    let memory = render_cold_html_svg(svg)?;
    let rss_after = current_process_rss_kib();

    assert_cold_html_font_memory(&memory);
    if let (Some(before), Some(after)) = (rss_before, rss_after) {
        assert_cold_html_rss(before, after, memory.owned_font_bytes);
    }
    Ok(())
}

fn render_cold_html_svg(svg: &str) -> Result<super::font::HtmlFontMemorySnapshot, String> {
    let options = html_rasterizer_options(svg);
    let memory = html_font_memory_snapshot(&options.fontdb);
    let tree = usvg::Tree::from_str(svg, &options).map_err(|error| error.to_string())?;
    let image = RasterTarget::new(tree.size(), 1.0)
        .render(&tree)
        .map_err(|error| error.to_string())?;
    drop(tree);
    drop(options);
    drop(image);
    Ok(memory)
}

fn assert_cold_html_font_memory(memory: &super::font::HtmlFontMemorySnapshot) {
    assert!(
        memory.owned_font_bytes <= 2 * 1024 * 1024,
        "owned font bytes: {}",
        memory.owned_font_bytes
    );
    assert!(
        memory.system_font_file_faces > 0,
        "CJK/emoji fallback must be registered as file-backed sources"
    );
    assert!(
        memory.total_faces <= 64,
        "lazy HTML database unexpectedly contains {} faces",
        memory.total_faces
    );
}

fn assert_cold_html_rss(before: usize, after: usize, owned_font_bytes: usize) {
    let delta = after.saturating_sub(before);
    eprintln!(
        "cold HTML font cache: owned={} B, rss_before={before} KiB, rss_after={after} KiB, delta={delta} KiB",
        owned_font_bytes
    );
    assert!(
        delta < 64 * 1024,
        "cold HTML RSS increased by {delta} KiB after file-backed font loading"
    );
}

#[test]
fn rasterize_svg_reports_parse_errors() {
    let image = SvgRasterizeOps::rasterize_svg("<svg>", 1.0);

    assert!(image.is_err());
}

#[test]
fn preprocess_handles_foreign_objects_entities_and_light_dark_colors() {
    let svg = r##"<svg fill="light-dark(#111, #eee)">&nbsp;<foreignObject><div>skip</div></foreignObject></svg>"##;
    let prepared = SvgRasterizeOps::preprocess_for_rasterizer(svg);
    let malformed = SvgRasterizeOps::preprocess_for_rasterizer(
        r##"<svg fill="light-dark(#111"><foreignObject><div></svg>"##,
    );
    let self_closed = SvgRasterizeOps::preprocess_for_rasterizer(r#"<svg><foreignObject /></svg>"#);

    assert!(prepared.contains("&#160;"));
    assert!(prepared.contains("#111"));
    assert!(!prepared.contains("foreignObject"));
    assert!(!self_closed.contains("foreignObject"));
    assert!(malformed.contains("light-dark("));
    assert!(malformed.contains("foreignObject"));
    assert_eq!(
        parse_light_dark_function("#123, rgb(1, 2, 3))"),
        Some((18, "#123"))
    );
    assert_eq!(parse_light_dark_function("#123"), None);
    assert!(effective_scale(10.0, 10.0, -1.0).is_sign_positive());
}

#[test]
fn zenuml_output_rasterizes_to_non_blank_image() {
    let Ok(svg) = render_zenuml_test_svg() else {
        return;
    };
    let image = SvgRasterizeOps::rasterize_svg(&svg, 1.0);
    assert!(
        image.as_ref().is_ok_and(|it| !it.rgba.is_empty()),
        "Rasterization failed: {image:?}"
    );
    assert!(
        image.as_ref().is_ok_and(|img| {
            !img.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[0] == 255 && p[1] == 255 && p[2] == 255)
        }),
        "Rasterized image is all white"
    );
}

fn render_zenuml_test_svg() -> Result<String, ()> {
    let mermaid = RuntimeAsset::mermaid();
    let mermaid_js = mermaid
        .materialize_at(mermaid.materialized_path())
        .map_err(|_| ())?;
    let block = DiagramBlock {
        kind: DiagramKind::Mermaid,
        source: "zenuml\ntitle Test\nA.method()".to_string(),
    };
    let DiagramResult::Ok(svg) = MermaidRenderOps::render_mermaid_with_runtime_path(
        &block,
        &mermaid_js,
        DiagramColorPreset::dark(),
    ) else {
        return Err(());
    };
    Ok(svg)
}

#[test]
fn raster_target_reports_pixmap_allocation_failure() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>"##;
    let tree = resvg::usvg::Tree::from_str(svg, &rasterizer_options());
    let target = RasterTarget {
        display_width: 1.0,
        display_height: 1.0,
        effective_scale: 1.0,
        width: 0,
        height: 0,
    };

    assert!(tree.as_ref().is_ok_and(|it| target.render(it).is_err()));
}
