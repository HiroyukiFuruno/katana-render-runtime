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
