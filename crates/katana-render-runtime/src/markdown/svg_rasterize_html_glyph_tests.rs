use super::super::composite::TextRasterizer;
use super::super::font_file_fixture;
use super::{ALPHA_CHANNEL, HtmlGlyphRasterizer, Prepared, RGBA_CHANNELS};
use resvg::usvg::{self, Node, Options, Tree};
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};

#[path = "svg_rasterize_html_glyph_origin_tests.rs"]
mod origin_tests;

#[path = "svg_rasterize_html_glyph_preparation_tests.rs"]
mod preparation_tests;

const FIXTURE_WIDTH: u32 = 160;
const FIXTURE_HEIGHT: u32 = 80;

fn parse_text(
    database: usvg::fontdb::Database,
    family: &str,
    weight: u16,
    stroke: bool,
) -> Result<(Tree, usize), String> {
    parse_text_content(database, family, weight, stroke, "Ag", "")
}

fn parse_text_content(
    database: usvg::fontdb::Database,
    family: &str,
    weight: u16,
    stroke: bool,
    content: &str,
    attributes: &str,
) -> Result<(Tree, usize), String> {
    let svg = fixture_svg(family, weight, stroke, content, attributes);
    let options = Options {
        fontdb: Arc::new(database),
        ..Options::default()
    };
    let tree = Tree::from_str(&svg, &options).map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("fixture text node missing".to_string());
    };
    let ids = text
        .layouted()
        .iter()
        .flat_map(|span| span.positioned_glyphs.iter())
        .map(|glyph| glyph.id.0)
        .collect::<Vec<_>>();
    if ids.is_empty() || ids.contains(&0) {
        return Err(format!("unexpected fixture glyph IDs: {ids:?}"));
    }
    Ok((tree, ids.len()))
}

fn fixture_svg(family: &str, weight: u16, stroke: bool, content: &str, attributes: &str) -> String {
    let stroke = if stroke { r#" stroke="black""# } else { "" };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{FIXTURE_WIDTH}" height="{FIXTURE_HEIGHT}">
          <text id="run" x="0" y="40" font-family="{family}" font-weight="{weight}" font-size="28" fill="black"{stroke}{attributes}>{content}</text>
        </svg>"#
    )
}

fn alpha_bounds(pixmap: &Pixmap) -> Option<(u32, u32)> {
    let mut min_x = pixmap.width();
    let mut max_x = 0;
    for (index, pixel) in pixmap.data().chunks_exact(RGBA_CHANNELS).enumerate() {
        if pixel[ALPHA_CHANNEL] == 0 {
            continue;
        }
        let x = index as u32 % pixmap.width();
        min_x = min_x.min(x);
        max_x = max_x.max(x);
    }
    (min_x <= max_x).then_some((min_x, max_x))
}

fn render_bounds(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &usvg::Text,
    transform: Transform,
    pixmap: &mut Pixmap,
) -> Result<Option<(u32, u32)>, String> {
    {
        let mut target = pixmap.as_mut();
        assert!(rasterizer.render(text, transform, &mut target)?);
    }
    Ok(alpha_bounds(pixmap))
}

fn assert_unsupported(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &usvg::Text,
    pixmap: &mut Pixmap,
) -> Result<(), String> {
    let before = pixmap.data().to_vec();
    {
        let mut target = pixmap.as_mut();
        assert!(!rasterizer.render(text, Transform::identity(), &mut target)?);
    }
    assert_eq!(before, pixmap.data());
    Ok(())
}

fn assert_glyph_count(text: &usvg::Text, expected: usize) {
    let actual = text
        .layouted()
        .iter()
        .flat_map(|span| span.positioned_glyphs.iter())
        .count();
    assert_eq!(actual, expected);
    assert!(actual > 0);
}

fn assert_alignment_ready(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &usvg::Text,
) -> Result<(), String> {
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("static fixture was not prepared".to_string());
    };
    assert_eq!(super::single_horizontal_alignment(text, &spans), Some(0.0));
    Ok(())
}

#[test]
fn static_file_face_renders_real_glyphs_and_reuses_face_cache() -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("krr-html-glyph-{}.ttf", std::process::id()));
    let fixture = font_file_fixture::create(path, font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())?;
    let (tree, expected_count) = parse_text(fixture.database.clone(), "Noto Sans", 400, false)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("fixture text node missing".to_string());
    };
    assert_glyph_count(text, expected_count);
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    assert_alignment_ready(&rasterizer, text)?;
    let mut pixmap =
        Pixmap::new(FIXTURE_WIDTH, FIXTURE_HEIGHT).ok_or("pixmap allocation failed")?;
    assert!(render_bounds(&rasterizer, text, Transform::identity(), &mut pixmap)?.is_some());
    assert_eq!(rasterizer.faces.borrow().len(), 1);
    assert!(rasterizer.font_mgr.get().is_some());
    let before_bounds = alpha_bounds(&pixmap);
    pixmap.fill(tiny_skia::Color::TRANSPARENT);
    let translated_bounds = render_bounds(
        &rasterizer,
        text,
        Transform::from_translate(20.0, 0.0),
        &mut pixmap,
    )?;
    assert_ne!(before_bounds, translated_bounds);
    assert!(translated_bounds.is_some_and(|(min_x, _)| min_x >= 19));
    assert_eq!(rasterizer.faces.borrow().len(), 1);
    Ok(())
}

#[test]
fn only_verified_html_text_skips_skia_font_preparation() -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("krr-html-route-{}.ttf", std::process::id()));
    let fixture = font_file_fixture::create(path, font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())?;
    let (tree, _) = parse_text(fixture.database.clone(), "Noto Sans", 400, false)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("route fixture text node missing".to_string());
    };
    for (registered_id, handled_by_skia) in [("run", false), ("unrelated", true)] {
        assert_text_route(&tree, text, registered_id, handled_by_skia)?;
    }
    Ok(())
}

fn assert_text_route(
    tree: &Tree,
    text: &usvg::Text,
    registered_id: &str,
    handled_by_skia: bool,
) -> Result<(), String> {
    let rasterizer = HtmlGlyphRasterizer::with_resvg_text(
        tree.fontdb(),
        [registered_id.to_string()].into_iter().collect(),
    );
    let mut pixmap =
        Pixmap::new(FIXTURE_WIDTH, FIXTURE_HEIGHT).ok_or("pixmap allocation failed")?;
    let handled = rasterizer.render(text, Transform::identity(), &mut pixmap.as_mut())?;
    assert_eq!(handled, handled_by_skia);
    assert_eq!(rasterizer.font_mgr.get().is_some(), handled_by_skia);
    assert_eq!(!rasterizer.faces.borrow().is_empty(), handled_by_skia);
    assert_eq!(alpha_bounds(&pixmap).is_some(), handled_by_skia);
    Ok(())
}

#[test]
fn unsupported_binary_face_and_stroke_leave_pixmap_unchanged() -> Result<(), String> {
    let mut database = usvg::fontdb::Database::new();
    database.load_font_data(font_file_fixture::FONT_BYTES.to_vec());
    let (tree, _) = parse_text(database, "Noto Sans", 400, false)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("fixture text node missing".to_string());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut pixmap =
        Pixmap::new(FIXTURE_WIDTH, FIXTURE_HEIGHT).ok_or("pixmap allocation failed")?;
    assert_unsupported(&rasterizer, text, &mut pixmap)?;
    assert!(rasterizer.font_mgr.get().is_none());

    let path =
        std::env::temp_dir().join(format!("krr-html-glyph-stroke-{}.ttf", std::process::id()));
    let fixture = font_file_fixture::create(path, font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())?;
    let (tree, _) = parse_text(fixture.database.clone(), "Noto Sans", 400, true)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("stroke fixture text node missing".to_string());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    assert_unsupported(&rasterizer, text, &mut pixmap)?;
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn mixed_explicit_tspan_glyph_runs_are_ready_for_skia_paint() -> Result<(), String> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="30"><text id="run" x="0" y="20" font-size="16"><tspan font-family="Trebuchet MS">ABC</tspan><tspan font-family="Hiragino Kaku Gothic ProN">動物</tspan></text></svg>"#;
    let mut options = usvg::Options {
        fontdb: super::super::font::html_font_db_for_text(
            "Trebuchet MS, Hiragino Kaku Gothic ProN",
            "ABC動物",
        ),
        ..usvg::Options::default()
    };
    super::super::text_shaping::install_html_font_resolver(&mut options);
    options.font_resolver.select_fallback = Box::new(|_, _, _| None);
    let tree = Tree::from_str(svg, &options).map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("mixed fallback text missing".to_string());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("mixed fallback text was rejected before painting".to_string());
    };
    let fonts = spans
        .iter()
        .map(|span| span.font)
        .collect::<std::collections::HashSet<_>>();
    assert!(spans.len() >= 2);
    assert!(fonts.len() >= 2);
    assert!(spans.iter().all(|span| span.x_alignment.is_none()));
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn valid_helvetica_faces_render_or_leave_pixels_for_resvg_fallback() -> Result<(), String> {
    let database = super::super::font::html_font_db_for_text("Helvetica", "Ag");
    for weight in [400, 700] {
        let (tree, _) = parse_text((*database).clone(), "Helvetica", weight, false)?;
        render_helvetica_case(&tree)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn render_helvetica_case(tree: &Tree) -> Result<(), String> {
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("Helvetica fixture text node missing".to_string());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut pixmap =
        Pixmap::new(FIXTURE_WIDTH, FIXTURE_HEIGHT).ok_or("pixmap allocation failed")?;
    let before = pixmap.data().to_vec();
    let result = {
        let mut target = pixmap.as_mut();
        rasterizer.render(text, Transform::identity(), &mut target)?
    };
    if result {
        assert!(alpha_bounds(&pixmap).is_some());
        return Ok(());
    }
    assert_eq!(before, pixmap.data());
    resvg::render(tree, Transform::identity(), &mut pixmap.as_mut());
    assert!(alpha_bounds(&pixmap).is_some());
    assert!(rasterizer.faces.borrow().values().any(Option::is_none));
    let fallback_pixels = pixmap.data().to_vec();
    assert!(!rasterizer.render(text, Transform::identity(), &mut pixmap.as_mut())?);
    assert_eq!(fallback_pixels, pixmap.data());
    Ok(())
}
