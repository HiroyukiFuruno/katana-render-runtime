use super::super::tests::FONT_BYTES;
use resvg::usvg;
use resvg::usvg::fontdb::{Database, ID, Stretch, Style, Weight};
use std::sync::Arc;

const FIRST_JAPANESE_FONT: &[u8] = include_bytes!(
    "../../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/MPLUS1p-Regular.ttf"
);
const SECOND_JAPANESE_FONT: &[u8] =
    include_bytes!("../../../../tests/fixtures/fonts/han-kerning/NotoSansCJKjp-han14.otf");
const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40"><text id="run" x="4" y="26" font-family="Noto Sans">Aあ</text></svg>"#;

#[test]
fn svg_fallback_skips_the_first_supporting_face_only_when_all_attributes_differ()
-> Result<(), String> {
    for (stretch, use_first) in [(Stretch::Condensed, false), (Stretch::Normal, true)] {
        let (database, primary, first, second) = registered_fonts(stretch)?;
        let expected = if use_first { first } else { second };
        assert_eq!(glyph_faces(database)?, [primary, expected], "{stretch:?}");
    }
    Ok(())
}

fn registered_fonts(stretch: Stretch) -> Result<(Database, ID, ID, ID), String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let primary = database.faces().next().ok_or("primary face missing")?.id;

    let mut first_source = Database::new();
    first_source.load_font_data(FIRST_JAPANESE_FONT.to_vec());
    let mut first = first_source
        .faces()
        .next()
        .cloned()
        .ok_or("first fallback face missing")?;
    first.style = Style::Italic;
    first.weight = Weight::BOLD;
    first.stretch = stretch;
    let first = database.push_face_info(first);

    database.load_font_data(SECOND_JAPANESE_FONT.to_vec());
    let second = database
        .faces()
        .last()
        .ok_or("second fallback face missing")?
        .id;
    assert_ne!(first, second);
    Ok((database, primary, first, second))
}

fn glyph_faces(database: Database) -> Result<Vec<ID>, String> {
    let options = usvg::Options {
        fontdb: Arc::new(database),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(SVG, &options).map_err(|error| error.to_string())?;
    let Some(usvg::Node::Text(text)) = tree.node_by_id("run") else {
        return Err("SVG text node missing".to_owned());
    };
    Ok(text
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .map(|glyph| glyph.font)
        .collect())
}
