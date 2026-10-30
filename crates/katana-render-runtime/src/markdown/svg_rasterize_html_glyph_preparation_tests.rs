use super::font_file_fixture;
use super::{FIXTURE_HEIGHT, FIXTURE_WIDTH, HtmlGlyphRasterizer, assert_unsupported};
use resvg::usvg::{Node, Options, Tree};
use std::sync::Arc;
use tiny_skia::Pixmap;

#[test]
fn unsupported_non_solid_fill_leaves_pixmap_unchanged() -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("krr-html-glyph-fill-{}.ttf", std::process::id()));
    let fixture = font_file_fixture::create(path, font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())?;
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="80"><text id="run" x="0" y="40" font-family="Noto Sans" font-size="28" fill="none">Ag</text></svg>"#;
    let tree = Tree::from_str(
        svg,
        &Options {
            fontdb: Arc::new(fixture.database.clone()),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("non-solid-fill text fixture missing".to_string());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut pixmap =
        Pixmap::new(FIXTURE_WIDTH, FIXTURE_HEIGHT).ok_or("pixmap allocation failed")?;
    assert_unsupported(&rasterizer, text, &mut pixmap)
}

#[test]
fn gradient_fill_falls_back_to_resvg_without_partial_glyph_paint() -> Result<(), String> {
    let path = std::env::temp_dir().join(format!(
        "krr-html-glyph-gradient-{}.ttf",
        std::process::id()
    ));
    let fixture = font_file_fixture::create(path, font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())?;
    let tree = gradient_tree(&fixture.database)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("gradient text fixture missing".to_owned());
    };
    assert_gradient_fallback(&tree, text)
}

fn assert_gradient_fallback(tree: &Tree, text: &resvg::usvg::Text) -> Result<(), String> {
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut pixmap =
        Pixmap::new(FIXTURE_WIDTH, FIXTURE_HEIGHT).ok_or("pixmap allocation failed")?;
    assert_unsupported(&rasterizer, text, &mut pixmap)?;
    resvg::render(tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    assert!(
        pixmap
            .data()
            .chunks_exact(super::RGBA_CHANNELS)
            .any(|pixel| pixel[super::ALPHA_CHANNEL] != 0)
    );
    Ok(())
}

fn gradient_tree(database: &resvg::usvg::fontdb::Database) -> Result<Tree, String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="80"><defs><linearGradient id="paint"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><text id="run" x="0" y="40" font-family="Noto Sans" font-size="28" fill="url(#paint)">Ag</text></svg>"##;
    Tree::from_str(
        svg,
        &Options {
            fontdb: Arc::new(database.clone()),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}
