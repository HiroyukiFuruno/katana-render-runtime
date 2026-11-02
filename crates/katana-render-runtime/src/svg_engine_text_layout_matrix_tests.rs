use crate::svg_engine::usvg::layout::DatabaseExt;
use crate::svg_engine::usvg::{self, Node, Options, Tree};
use read_fonts::{FontRef, TableProvider};
use rstest::rstest;
use skrifa::MetadataProvider;
use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "svg_engine_text_layout_path_rotation_tests.rs"]
mod path_rotation_tests;
#[path = "svg_engine_text_layout_path_support.rs"]
mod path_support;
#[path = "svg_engine_text_layout_position_tests.rs"]
mod position_tests;
#[path = "svg_engine_text_layout_spacing_tests.rs"]
mod spacing_tests;
#[path = "svg_engine_text_layout_variation_tests.rs"]
mod variation_tests;

const FONT: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
const TOLERANCE: f32 = 0.001;
const UPM_FIELD_BYTES: usize = 2;
const FONT_FACE_COUNT: usize = 1;
const FONT_FACE_INDEX: u32 = 0;
const UPM_ZERO: u16 = 0;
const UPM_BELOW_MINIMUM: u16 = 15;
const UPM_ABOVE_MAXIMUM: u16 = 16_385;
const MISSING: char = '\u{10fffd}';
const INPUT: &str = "A\u{10fffd}B";
type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

fn parse(attributes: &str, content: &str) -> Result<Tree, String> {
    let mut database = usvg::fontdb::Database::new();
    database.load_font_data(FONT.to_vec());
    let options = Options {
        fontdb: Arc::new(database),
        ..Options::default()
    };
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='200' height='100'><text id='sample' x='20' y='50' font-family='Noto Sans' font-size='20' {attributes}>{content}</text></svg>"
    );
    Tree::from_str(&svg, &options).map_err(|error| error.to_string())
}

fn text(tree: &Tree) -> Result<&usvg::Text, String> {
    match tree.node_by_id("sample") {
        Some(Node::Text(text)) => Ok(text),
        _ => Err("text missing".to_owned()),
    }
}

fn near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < TOLERANCE,
        "{actual} != {expected}"
    );
}

#[rstest]
#[case::zero(UPM_ZERO)]
#[case::below_minimum(UPM_BELOW_MINIMUM)]
#[case::above_maximum(UPM_ABOVE_MAXIMUM)]
fn invalid_units_per_em_is_rejected_by_layout_and_font_loading(
    #[case] invalid_upm: u16,
) -> TestResult<()> {
    let requested_font = requested_font()?;
    let (mut valid_database, valid_id) = database_with_font(FONT.to_vec())?;
    assert_font_metrics_accepted(&mut valid_database, valid_id, &requested_font)?;

    let malformed = font_with_units_per_em(invalid_upm)?;
    let (mut malformed_database, malformed_id) = database_with_font(malformed)?;
    assert_font_metrics_rejected(&mut malformed_database, malformed_id, &requested_font);
    Ok(())
}

fn requested_font() -> TestResult<usvg::Font> {
    let tree = parse("", "x")?;
    let span = text(&tree)?
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .ok_or(std::io::Error::other("text span missing"))?;
    Ok(span.font().clone())
}

fn database_with_font(
    bytes: Vec<u8>,
) -> TestResult<(Arc<usvg::fontdb::Database>, usvg::fontdb::ID)> {
    let mut database = usvg::fontdb::Database::new();
    let ids = database.load_font_source(usvg::fontdb::Source::Binary(Arc::new(bytes)));
    assert_eq!(
        ids.len(),
        FONT_FACE_COUNT,
        "Noto Sans face was not registered"
    );
    let id = *ids
        .first()
        .ok_or(std::io::Error::other("Noto Sans face ID missing"))?;
    let families = [usvg::fontdb::Family::Name("Noto Sans")];
    let query = usvg::fontdb::Query {
        families: &families,
        ..usvg::fontdb::Query::default()
    };
    assert_eq!(database.query(&query), Some(id));
    Ok((Arc::new(database), id))
}

fn assert_font_metrics_accepted(
    database: &mut Arc<usvg::fontdb::Database>,
    id: usvg::fontdb::ID,
    font: &usvg::Font,
) -> TestResult<()> {
    assert!(database.load_font(id, &[]).is_some());
    let resolver = usvg::FontResolver::default();
    assert_eq!((resolver.select_font)(font, database), Some(id));
    let x_height = usvg::layout::css_font_x_height(font, &resolver, database)
        .ok_or(std::io::Error::other("valid font x-height missing"))?;
    assert!(x_height.is_finite() && x_height > 0.0);
    Ok(())
}

fn assert_font_metrics_rejected(
    database: &mut Arc<usvg::fontdb::Database>,
    id: usvg::fontdb::ID,
    font: &usvg::Font,
) {
    assert!(database.load_font(id, &[]).is_none());
    let resolver = usvg::FontResolver::default();
    assert_eq!((resolver.select_font)(font, database), Some(id));
    assert!(usvg::layout::css_font_x_height(font, &resolver, database).is_none());
}

fn font_with_units_per_em(units_per_em: u16) -> TestResult<Vec<u8>> {
    let mut bytes = FONT.to_vec();
    let field = units_per_em_range(&bytes)?;
    let destination = bytes
        .get_mut(field.clone())
        .ok_or(std::io::Error::other("head.unitsPerEm field missing"))?;
    assert_eq!(destination.len(), UPM_FIELD_BYTES);
    destination.copy_from_slice(&units_per_em.to_be_bytes());
    assert_only_upm_field_changed(&bytes, field);
    assert_skrifa_units_per_em(&bytes, units_per_em)?;
    Ok(bytes)
}

fn assert_skrifa_units_per_em(bytes: &[u8], expected: u16) -> TestResult<()> {
    let font = skrifa::FontRef::from_index(bytes, FONT_FACE_INDEX)?;
    let location = font.axes().location(std::iter::empty::<(&str, f32)>());
    let actual = font
        .metrics(skrifa::prelude::Size::unscaled(), &location)
        .units_per_em;
    assert_eq!(actual, expected);
    Ok(())
}

fn units_per_em_range(bytes: &[u8]) -> TestResult<std::ops::Range<usize>> {
    let font = FontRef::new(bytes)?;
    let head_offset = font
        .table_directory()
        .table_records()
        .iter()
        .find(|record| record.tag() == read_fonts::types::Tag::new(b"head"))
        .map(|record| record.offset() as usize)
        .ok_or(std::io::Error::other("head table missing"))?;
    let field = font.head()?.units_per_em_byte_range();
    Ok((head_offset + field.start)..(head_offset + field.end))
}

fn assert_only_upm_field_changed(bytes: &[u8], field: std::ops::Range<usize>) {
    let changed_positions = bytes
        .iter()
        .zip(FONT)
        .enumerate()
        .filter_map(|(position, (actual, original))| (actual != original).then_some(position))
        .collect::<Vec<_>>();
    assert_eq!(
        changed_positions,
        (field.start..field.end).collect::<Vec<_>>()
    );
}

#[test]
fn missing_glyph_without_fallback_keeps_cluster_text_and_position() -> TestResult<()> {
    crate::svg_engine::init_test_logger();
    assert_missing_from_font()?;
    let requested = requested_font()?;
    let (mut database, id) = database_with_font(FONT.to_vec())?;
    let resolved = database
        .load_font(id, &[])
        .ok_or(std::io::Error::other("font missing"))?;
    let calls = Arc::new(AtomicUsize::new(0));
    let resolver = no_fallback_resolver(id, Arc::clone(&calls));
    let glyphs = usvg::layout::shape_text(
        INPUT,
        Arc::new(resolved),
        false,
        false,
        &requested,
        20.0,
        usvg::FontOpticalSizing::Auto,
        &resolver,
        &mut database,
    );
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_missing_glyphs(&glyphs)
}

fn assert_missing_from_font() -> TestResult<()> {
    let face = skrifa::FontRef::from_index(FONT, FONT_FACE_INDEX)?;
    assert!(
        !face
            .charmap()
            .mappings()
            .any(|(codepoint, _)| codepoint == MISSING as u32)
    );
    Ok(())
}

fn no_fallback_resolver(
    id: usvg::fontdb::ID,
    count: Arc<AtomicUsize>,
) -> usvg::FontResolver<'static> {
    usvg::FontResolver {
        select_fallback: Box::new(move |character, used, _| {
            assert_eq!(character, MISSING);
            assert_eq!(used, &[id]);
            count.fetch_add(1, Ordering::Relaxed);
            None
        }),
        ..usvg::FontResolver::default()
    }
}

fn assert_missing_glyphs(glyphs: &[usvg::layout::Glyph]) -> TestResult<()> {
    let [first, missing, last] = glyphs else {
        return Err(std::io::Error::other("expected three glyph clusters").into());
    };
    assert_ne!(first.id.0, 0);
    assert_eq!(missing.id.0, 0);
    assert_ne!(last.id.0, 0);
    assert_glyph_text_and_positions(first, missing, last);
    Ok(())
}

fn assert_glyph_text_and_positions(
    first: &usvg::layout::Glyph,
    missing: &usvg::layout::Glyph,
    last: &usvg::layout::Glyph,
) {
    assert_eq!(
        (
            first.text.as_str(),
            missing.text.as_str(),
            last.text.as_str()
        ),
        ("A", "\u{10fffd}", "B")
    );
    assert_eq!(
        (
            first.byte_idx.value(),
            missing.byte_idx.value(),
            last.byte_idx.value()
        ),
        (0, 1, 1 + MISSING.len_utf8())
    );
    assert!(first.width > 0 && missing.width > 0 && last.width > 0);
    assert_eq!((missing.dx, missing.dy), (0, 0));
}
