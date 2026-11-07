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
    let primary = database
        .load_font(primary_id, &[])
        .ok_or(std::io::Error::other("primary font could not be loaded"))?;
    let mut database = Arc::new(database);
    let resolver = usvg::FontResolver {
        select_fallback: Box::new(move |character, used, _| {
            (character == DEVANAGARI_KA
                && used.contains(&primary_id)
                && !used.contains(&fallback_id))
            .then_some(fallback_id)
        }),
        ..usvg::FontResolver::default()
    };
    let glyphs = usvg::layout::shape_text(
        text,
        Arc::new(primary),
        false,
        false,
        &requested_font()?,
        FONT_SIZE,
        usvg::FontOpticalSizing::Auto,
        &resolver,
        &mut database,
    );
    Ok((glyphs, primary_id, fallback_id))
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
