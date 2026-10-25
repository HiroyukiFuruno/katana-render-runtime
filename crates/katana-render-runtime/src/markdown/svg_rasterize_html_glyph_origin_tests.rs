use super::{HtmlGlyphRasterizer, Prepared};
use resvg::usvg::Node;

#[test]
fn run_origin_snap_requires_simple_one_to_one_multi_glyph_text() -> Result<(), String> {
    let path =
        std::env::temp_dir().join(format!("krr-html-origin-snap-{}.ttf", std::process::id()));
    let fixture = super::font_file_fixture::create(path, super::font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())?;
    for (content, attributes, expected) in [
        ("Ag", "", true),
        ("A", "", false),
        ("Ag", r#" dx="1""#, false),
    ] {
        assert_eligibility(fixture.database.clone(), content, attributes, expected)?;
    }
    Ok(())
}

fn assert_eligibility(
    database: resvg::usvg::fontdb::Database,
    content: &str,
    attributes: &str,
    expected: bool,
) -> Result<(), String> {
    let (tree, _) = super::parse_text_content(
        database,
        "Noto Sans",
        resvg::usvg::fontdb::Weight::NORMAL.0,
        false,
        content,
        attributes,
    )?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("origin-snap fixture text missing".to_owned());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("origin-snap fixture was not prepared".to_owned());
    };
    let origin_css_scale = spans
        .first()
        .and_then(|span| span.x_alignment)
        .and_then(|alignment| alignment.origin_snap_css_scale);
    assert_eq!(origin_css_scale.is_some(), expected);
    Ok(())
}
