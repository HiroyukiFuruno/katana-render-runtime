use super::{near, text, usvg};
use rstest::rstest;
use std::sync::Arc;

const SANS: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
const MONO: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/NotoMono-Regular.ttf"
);

#[rstest]
#[case::ligature("fi")]
#[case::combining_mark("D\u{0323}")]
fn changing_span_font_preserves_the_individual_glyphs(#[case] cluster: &str) -> Result<(), String> {
    let mut database = usvg::fontdb::Database::new();
    database.load_font_data(SANS.to_vec());
    database.load_font_data(MONO.to_vec());
    let options = usvg::Options {
        fontdb: Arc::new(database),
        ..Default::default()
    };
    let span = format!("<tspan font-family='Noto Mono'>{cluster}</tspan>");
    let sans = parse(&options, &format!("x{cluster}"))?;
    let mono = parse(&options, &span)?;
    let mixed = parse(&options, &format!("x{span}"))?;
    assert_mixed_glyphs(&glyphs(&sans)?, &glyphs(&mono)?, &glyphs(&mixed)?);
    Ok(())
}

fn assert_mixed_glyphs(
    sans: &[&usvg::layout::PositionedGlyph],
    mono: &[&usvg::layout::PositionedGlyph],
    mixed: &[&usvg::layout::PositionedGlyph],
) {
    assert_eq!(sans.len(), 2);
    assert_eq!(mono.len(), 2);
    assert_eq!(mixed.len(), 3);
    assert_eq!((mixed[0].font, mixed[0].id), (sans[0].font, sans[0].id));
    assert_ne!(mixed[0].font, mono[0].font);
    for (actual, expected) in mixed[1..].iter().zip(mono) {
        assert_eq!((actual.font, actual.id), (expected.font, expected.id));
    }
    near(
        mixed[2].transform().tx - mixed[1].transform().tx,
        mono[1].transform().tx - mono[0].transform().tx,
    );
}

fn parse(options: &usvg::Options<'_>, content: &str) -> Result<usvg::Tree, String> {
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='200' height='100'><text id='sample' x='20' y='50' font-family='Noto Sans' font-size='20'>{content}</text></svg>"
    );
    usvg::Tree::from_str(&svg, options).map_err(|error| error.to_string())
}

fn glyphs(tree: &usvg::Tree) -> Result<Vec<&usvg::layout::PositionedGlyph>, String> {
    Ok(text(tree)?
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .collect())
}
