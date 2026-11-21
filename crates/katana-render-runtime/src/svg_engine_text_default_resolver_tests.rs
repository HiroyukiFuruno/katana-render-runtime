use super::{FONT, TestResult, font_fixture_tests::fixture_database, requested_font, usvg};
use crate::svg_engine::usvg::layout::DatabaseExt;
use rstest::rstest;
use std::sync::Arc;

const FALLBACK_FONT: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/Yellowtail-Regular.ttf"
);

type GenericFamilySetter = fn(&mut usvg::fontdb::Database);

#[rstest]
#[case::serif(usvg::FontFamily::Serif, set_serif_family)]
#[case::sans(usvg::FontFamily::SansSerif, set_sans_serif_family)]
#[case::cursive(usvg::FontFamily::Cursive, set_cursive_family)]
#[case::fantasy(usvg::FontFamily::Fantasy, set_fantasy_family)]
#[case::monospace(usvg::FontFamily::Monospace, set_monospace_family)]
fn default_selector_maps_each_generic_family(
    #[case] family: usvg::FontFamily,
    #[case] set_family: GenericFamilySetter,
) -> TestResult<()> {
    assert_family_selects_fixture(family, set_family)
}

fn assert_family_selects_fixture(
    family: usvg::FontFamily,
    set_family: GenericFamilySetter,
) -> TestResult<()> {
    let (mut database, fixture_id) = fixture_database(FONT)?;
    set_family(Arc::make_mut(&mut database));
    assert_selected_family(family, &mut database, fixture_id)
}

fn assert_named_family_selects_fixture(family: usvg::FontFamily) -> TestResult<()> {
    let (mut database, fixture_id) = fixture_database(FONT)?;
    assert_selected_family(family, &mut database, fixture_id)
}

#[test]
fn default_selector_uses_registered_named_family() -> TestResult<()> {
    assert_eq!(
        format!("{:?}", usvg::FontResolver::default()),
        "FontResolver { .. }"
    );
    assert_named_family_selects_fixture(usvg::FontFamily::Named("Noto Sans".into()))
}

#[test]
fn default_selector_uses_serif_after_unregistered_named_family() -> TestResult<()> {
    let (mut database, fixture_id) = fixture_database(FONT)?;
    Arc::make_mut(&mut database).set_serif_family("Noto Sans");
    assert_selected_family(
        usvg::FontFamily::Named("unregistered".into()),
        &mut database,
        fixture_id,
    )
}

fn assert_selected_family(
    family: usvg::FontFamily,
    database: &mut Arc<usvg::fontdb::Database>,
    fixture_id: usvg::fontdb::ID,
) -> TestResult<()> {
    let mut requested = requested_font()?;
    requested.families = vec![family];
    let resolver = usvg::FontResolver::default();
    assert_eq!(
        (resolver.select_font)(&requested, database),
        Some(fixture_id)
    );
    Ok(())
}

fn set_serif_family(database: &mut usvg::fontdb::Database) {
    database.set_serif_family("Noto Sans");
}

fn set_sans_serif_family(database: &mut usvg::fontdb::Database) {
    database.set_sans_serif_family("Noto Sans");
}

fn set_cursive_family(database: &mut usvg::fontdb::Database) {
    database.set_cursive_family("Noto Sans");
}

fn set_fantasy_family(database: &mut usvg::fontdb::Database) {
    database.set_fantasy_family("Noto Sans");
}

fn set_monospace_family(database: &mut usvg::fontdb::Database) {
    database.set_monospace_family("Noto Sans");
}

#[test]
fn default_selector_rejects_empty_database() -> TestResult<()> {
    crate::svg_engine::init_test_logger();
    let requested = requested_font()?;
    let mut database = Arc::new(usvg::fontdb::Database::new());
    let resolver = usvg::FontResolver::default();
    assert_eq!((resolver.select_font)(&requested, &mut database), None);
    Ok(())
}

#[test]
fn default_fallback_rejects_removed_base_and_selects_with_live_base() -> TestResult<()> {
    crate::svg_engine::init_test_logger();
    let (mut database, base_id) = fixture_database(FONT)?;
    let fallback_ids = Arc::make_mut(&mut database).load_font_source(usvg::fontdb::Source::Binary(
        Arc::new(FALLBACK_FONT.to_vec()),
    ));
    let [fallback_id] = fallback_ids.as_slice() else {
        return Err("expected one fallback fixture face".into());
    };
    let fallback_id = *fallback_id;
    assert!(database.has_char(base_id, 'A'));
    assert!(database.has_char(fallback_id, 'A'));
    let select_fallback = usvg::FontResolver::default_fallback_selector();
    assert_eq!(
        select_fallback('A', &[base_id], &mut database),
        Some(fallback_id)
    );

    Arc::make_mut(&mut database).remove_face(base_id);
    assert_eq!(select_fallback('A', &[base_id], &mut database), None);
    Ok(())
}
