use super::{cache, entries, scope};
use resvg::usvg::fontdb::{Database, Source};
use std::sync::Arc;

const FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

fn database() -> Arc<Database> {
    let mut database = Database::new();
    database.load_font_source(Source::Binary(Arc::new(FONT.to_vec())));
    Arc::new(database)
}

fn with_memo<T>(mut operation: impl FnMut() -> T) -> T {
    super::super::font::with_validated_stamp_batch(|| scope::with_attempt(&mut operation))
}

#[test]
fn lookup_and_store_bypass_cleanly_without_an_active_attempt() {
    let database = database();
    assert!(matches!(
        cache::lookup(&database, 'x', &[]),
        cache::Lookup::Bypass
    ));
    let key = entries::selector_key('x', &[]);
    assert!(matches!(
        entries::cached_lookup(&database, &key, entries::key_bytes(&key)),
        entries::CachedLookup::Bypass
    ));
    entries::add_database(&database);
    entries::store(&database, key, None);
    assert!(scope::ACTIVE_MEMO.with(|active| active.borrow().is_none()));
}

#[test]
fn stamp_batch_without_selector_scope_does_not_store_results() {
    let database = database();
    super::super::font::with_validated_stamp_batch(|| {
        assert!(scope::ACTIVE_MEMO.with(|active| active.borrow().is_none()));
        entries::store(&database, entries::selector_key('x', &[]), None);
        assert!(scope::ACTIVE_MEMO.with(|active| active.borrow().is_none()));
    });
}

#[test]
fn observed_database_populates_one_key_and_reuses_none_results() {
    let database = database();
    with_memo(|| {
        let key = entries::selector_key('x', &[]);
        let bytes = entries::key_bytes(&key);
        assert!(matches!(
            entries::cached_lookup(&database, &key, bytes),
            entries::CachedLookup::ObserveDatabase
        ));
        entries::add_database(&database);
        assert!(matches!(
            entries::cached_lookup(&database, &key, bytes),
            entries::CachedLookup::Miss
        ));
        entries::store(&database, key, None);
        let stored = entries::selector_key('x', &[]);
        assert!(matches!(
            entries::cached_lookup(&database, &stored, bytes),
            entries::CachedLookup::Hit(None)
        ));
    });
}

#[test]
fn copy_on_write_database_identity_does_not_reuse_the_old_partition() {
    let mut database = database();
    with_memo(|| {
        let key = entries::selector_key('x', &[]);
        entries::add_database(&database);
        entries::store(&database, key, None);
        Arc::make_mut(&mut database).load_font_source(Source::Binary(Arc::new(FONT.to_vec())));
        let changed = entries::selector_key('x', &[]);
        entries::store(&database, changed, None);
        let changed = entries::selector_key('x', &[]);
        assert!(matches!(
            entries::cached_lookup(&database, &changed, entries::key_bytes(&changed)),
            entries::CachedLookup::ObserveDatabase
        ));
    });
}

#[test]
fn full_entry_budget_rejects_new_keys() {
    let database = database();
    with_memo(|| {
        entries::add_database(&database);
        for character in (0x1000..)
            .filter_map(char::from_u32)
            .take(entries::MAX_ENTRIES)
        {
            let key = entries::selector_key(character, &[]);
            entries::store(&database, key, None);
        }
        let rejected = entries::selector_key('z', &[]);
        entries::store(&database, rejected, None);
        let rejected = entries::selector_key('z', &[]);
        assert!(matches!(
            entries::cached_lookup(&database, &rejected, entries::key_bytes(&rejected)),
            entries::CachedLookup::Bypass
        ));
    });
}

#[test]
fn styled_keys_distinguish_requested_weight_style_and_stretch() -> Result<(), String> {
    let database = database();
    let tree = styled_fixture(&database)?;
    let fonts = requested_fonts(&tree)?;
    assert_eq!(fonts.len(), 5);
    let keys: Vec<_> = fonts
        .iter()
        .map(|font| entries::styled_selector_key(font, 'x', &[]))
        .collect();
    for (index, key) in keys.iter().enumerate() {
        assert!(keys.iter().skip(index + 1).all(|other| key != other));
    }
    Ok(())
}

#[test]
fn styled_keys_remain_bound_to_database_generation() -> Result<(), String> {
    let mut database = database();
    let tree = styled_fixture(&database)?;
    let fonts = requested_fonts(&tree)?;
    with_memo(|| {
        let key = entries::styled_selector_key(fonts[0], 'x', &[]);
        entries::add_database(&database);
        entries::store(&database, key, None);
        Arc::make_mut(&mut database).load_font_source(Source::Binary(Arc::new(FONT.to_vec())));
        let changed = entries::styled_selector_key(fonts[0], 'x', &[]);
        assert!(matches!(
            entries::cached_lookup(&database, &changed, entries::key_bytes(&changed)),
            entries::CachedLookup::ObserveDatabase
        ));
    });
    Ok(())
}

#[test]
fn styled_selector_rejects_missing_base_identity() -> Result<(), String> {
    let database = database();
    let tree = styled_fixture(&database)?;
    let fonts = requested_fonts(&tree)?;
    let mut options = resvg::usvg::Options {
        fontdb: Arc::clone(&database),
        ..resvg::usvg::Options::default()
    };
    super::super::install_html_font_resolver(&mut options);
    let selector = options
        .font_resolver
        .select_styled_fallback
        .as_ref()
        .ok_or("styled selector missing")?;
    let mut requested_database = Arc::clone(&database);
    assert!(selector(fonts[0], 'x', &[], &mut requested_database).is_none());
    Ok(())
}

fn styled_fixture(database: &Arc<Database>) -> Result<resvg::usvg::Tree, String> {
    let svg = "<svg xmlns='http://www.w3.org/2000/svg'><text id='x' font-family='Noto Sans'><tspan font-weight='400'>A</tspan><tspan font-weight='600'>A</tspan><tspan font-weight='700'>A</tspan><tspan font-style='italic'>A</tspan><tspan font-stretch='condensed'>A</tspan></text></svg>";
    let options = resvg::usvg::Options {
        fontdb: Arc::clone(database),
        ..resvg::usvg::Options::default()
    };
    resvg::usvg::Tree::from_str(svg, &options).map_err(|error| error.to_string())
}

fn requested_fonts(tree: &resvg::usvg::Tree) -> Result<Vec<&resvg::usvg::Font>, String> {
    let Some(resvg::usvg::Node::Text(text)) = tree.node_by_id("x") else {
        return Err("fixture text missing".into());
    };
    Ok(text
        .chunks()
        .iter()
        .flat_map(|chunk| chunk.spans())
        .map(|span| span.font())
        .collect())
}
