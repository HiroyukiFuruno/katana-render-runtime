use super::{TestResult, requested_font};
use crate::svg_engine::usvg::{self, layout::DatabaseExt};
use std::sync::Arc;

const PRIMARY: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/Yellowtail-Regular.ttf"
);
const FALLBACK: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/NotoSansDevanagari-Regular.ttf"
);
const DEVANAGARI_KA: char = '\u{0915}';
const MISSING_CHARACTER: char = '\u{10fffd}';
const FONT_SIZE: f32 = 20.0;

fn fallback_font_database() -> (usvg::fontdb::Database, usvg::fontdb::ID, usvg::fontdb::ID) {
    let mut database = usvg::fontdb::Database::new();
    let primary_ids =
        database.load_font_source(usvg::fontdb::Source::Binary(Arc::new(PRIMARY.to_vec())));
    let fallback_ids =
        database.load_font_source(usvg::fontdb::Source::Binary(Arc::new(FALLBACK.to_vec())));
    assert_eq!(primary_ids.len(), 1);
    assert_eq!(fallback_ids.len(), 1);
    let (primary_id, fallback_id) = (primary_ids[0], fallback_ids[0]);
    assert!(!database.has_char(primary_id, DEVANAGARI_KA));
    assert!(database.has_char(fallback_id, DEVANAGARI_KA));
    assert!(!database.has_char(fallback_id, MISSING_CHARACTER));
    (database, primary_id, fallback_id)
}

fn shape(text: &str) -> TestResult<(Vec<usvg::layout::Glyph>, usvg::fontdb::ID, usvg::fontdb::ID)> {
    let (database, primary_id, fallback_id) = fallback_font_database();
    let resolver = usvg::FontResolver {
        select_fallback: Box::new(move |character, used, _| {
            (character == DEVANAGARI_KA
                && used.contains(&primary_id)
                && !used.contains(&fallback_id))
            .then_some(fallback_id)
        }),
        ..usvg::FontResolver::default()
    };
    let glyphs = shape_with_font(text, database, primary_id, &resolver)?;
    Ok((glyphs, primary_id, fallback_id))
}

fn shape_with_font(
    text: &str,
    database: usvg::fontdb::Database,
    id: usvg::fontdb::ID,
    resolver: &usvg::FontResolver,
) -> TestResult<Vec<usvg::layout::Glyph>> {
    let font = database
        .load_font(id, &[])
        .ok_or(std::io::Error::other("test font could not be loaded"))?;
    Ok(usvg::layout::shape_text(
        text,
        Arc::new(font),
        false,
        false,
        &requested_font()?,
        FONT_SIZE,
        usvg::FontOpticalSizing::Auto,
        resolver,
        &mut Arc::new(database),
    ))
}

#[test]
fn fallback_replaces_differently_shaped_indic_clusters() -> TestResult<()> {
    crate::svg_engine::init_test_logger();
    let text = "\u{0915}\u{094d}\u{0937}";
    let (database, primary_id, fallback_id) = fallback_font_database();
    let resolver = usvg::FontResolver {
        select_fallback: Box::new(|_, _, _| None),
        ..usvg::FontResolver::default()
    };
    let primary = shape_with_font(text, database.clone(), primary_id, &resolver)?;
    let expected = shape_with_font(text, database, fallback_id, &resolver)?;
    let boundaries = |glyphs: &[usvg::layout::Glyph]| {
        glyphs
            .iter()
            .map(|glyph| glyph.byte_idx.value())
            .collect::<Vec<_>>()
    };
    assert_ne!(boundaries(&primary), boundaries(&expected));
    assert!(primary.iter().any(|glyph| glyph.id.0 == 0));
    assert!(!expected.is_empty());
    let (actual, _, fallback_id) = shape(text)?;
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(&expected) {
        assert_ne!(expected.id.0, 0);
        assert_eq!(actual.id.0, expected.id.0);
        assert_eq!(actual.text, expected.text);
        assert_eq!(actual.byte_idx.value(), expected.byte_idx.value());
        assert_eq!(actual.font.id, fallback_id);
    }
    Ok(())
}

#[test]
fn fully_matched_fallback_replaces_primary_cluster() -> TestResult<()> {
    crate::svg_engine::init_test_logger();
    let (glyphs, _, fallback_id) = shape("\u{0915}")?;
    let [glyph] = glyphs.as_slice() else {
        return Err(std::io::Error::other("expected one glyph cluster").into());
    };
    assert_eq!(glyph.text, "\u{0915}");
    assert_eq!(glyph.byte_idx.value(), 0);
    assert_ne!(glyph.id.0, 0);
    assert_eq!(glyph.font.id, fallback_id);
    Ok(())
}

#[test]
fn partial_fallback_preserves_unmatched_primary_cluster() -> TestResult<()> {
    crate::svg_engine::init_test_logger();
    let text = "\u{0915}\u{10fffd}";
    let (glyphs, primary_id, fallback_id) = shape(text)?;
    let [matched, missing] = glyphs.as_slice() else {
        return Err(std::io::Error::other("expected two glyph clusters").into());
    };
    assert_eq!(
        (matched.text.as_str(), matched.byte_idx.value()),
        ("\u{0915}", 0)
    );
    assert_ne!(matched.id.0, 0);
    assert_eq!(matched.font.id, fallback_id);
    assert_eq!(missing.text, "\u{10fffd}");
    assert_eq!(missing.byte_idx.value(), "\u{0915}".len());
    assert_eq!(missing.id.0, 0);
    assert_eq!(missing.font.id, primary_id);
    Ok(())
}

#[test]
fn partial_indic_fallback_preserves_an_existing_latin_glyph_and_missing_character() -> TestResult<()>
{
    crate::svg_engine::init_test_logger();
    /* WHY: 最初の文字がシェーピングの用字系を決めるため、Indic合字を先に置く。 */
    let text = "\u{0915}\u{094d}\u{0937}A\u{10fffd}";
    let (primary, primary_id) = verify_partial_indic_shaping(text)?;
    let primary_latin = primary
        .iter()
        .find(|glyph| glyph.text == "A")
        .ok_or("primary A missing")?;
    assert_ne!(primary_latin.id.0, 0);
    let (actual, _, _) = shape(text)?;
    let actual_latin = actual
        .iter()
        .find(|glyph| glyph.text == "A")
        .ok_or("actual A missing")?;
    assert_eq!(actual_latin.id, primary_latin.id);
    assert_eq!(actual_latin.font.id, primary_id);
    assert!(
        actual
            .iter()
            .any(|glyph| glyph.text.contains(MISSING_CHARACTER) && glyph.id.0 == 0)
    );
    Ok(())
}

fn verify_partial_indic_shaping(
    text: &str,
) -> TestResult<(Vec<usvg::layout::Glyph>, usvg::fontdb::ID)> {
    let (database, primary_id, fallback_id) = fallback_font_database();
    let no_fallback = usvg::FontResolver {
        select_fallback: Box::new(|_, _, _| None),
        ..usvg::FontResolver::default()
    };
    let primary = shape_with_font(text, database.clone(), primary_id, &no_fallback)?;
    let fallback = shape_with_font(text, database, fallback_id, &no_fallback)?;
    assert_ne!(
        primary.len(),
        fallback.len(),
        "fixture must shape differently"
    );
    assert!(fallback.iter().any(|glyph| glyph.id.0 == 0));
    Ok((primary, primary_id))
}
