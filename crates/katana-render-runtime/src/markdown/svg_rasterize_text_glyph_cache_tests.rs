use super::{
    FileStamp, GLYPH_CACHE, GlyphCacheEntry, GlyphKey, MAX_DATABASES, MAX_FILE_STAMPS, MAX_GLYPHS,
    cached_font_has_char, insert, lookup,
};
use crate::markdown::svg_rasterize::font::bundled_font_db;
use resvg::usvg::fontdb::{Database, ID, Source};
use std::collections::HashMap;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicUsize, Ordering},
};

struct CountedBytes {
    bytes: Vec<u8>,
    reads: AtomicUsize,
}

impl AsRef<[u8]> for CountedBytes {
    fn as_ref(&self) -> &[u8] {
        self.reads.fetch_add(1, Ordering::Relaxed);
        assert!(GLYPH_CACHE.with(|cache| cache.try_borrow_mut().is_ok()));
        &self.bytes
    }
}

type CountedDatabase = (Arc<Database>, ID, Arc<CountedBytes>);
fn counted_database(valid: bool) -> Result<CountedDatabase, String> {
    let bundled = bundled_font_db();
    let mut face = bundled
        .faces()
        .next()
        .ok_or("bundled face missing")?
        .clone();
    let bytes = bundled
        .with_face_data(face.id, |data, _| data.to_vec())
        .ok_or("bundled bytes missing")?;
    let data = Arc::new(CountedBytes {
        bytes: if valid { bytes } else { vec![0; 32] },
        reads: AtomicUsize::new(0),
    });
    face.source = Source::Binary(data.clone());
    let mut database = Database::new();
    let id = database.push_face_info(face);
    Ok((Arc::new(database), id, data))
}

#[test]
fn valid_positive_and_negative_probes_are_reused_without_reading_bytes() -> Result<(), String> {
    let (database, id, data) = counted_database(true)?;
    assert!(
        super::super::file_generation::font_source_generation(&database, id).durable_reusable()
    );
    for (ch, expected) in [('A', true), ('\u{10ffff}', false)] {
        assert_eq!(cached_font_has_char(&database, id, ch), Some(expected));
        let reads = data.reads.load(Ordering::Relaxed);
        assert_eq!(cached_font_has_char(&database, id, ch), Some(expected));
        assert_eq!(data.reads.load(Ordering::Relaxed), reads);
    }
    Ok(())
}

#[test]
fn glyph_support_survives_the_old_4096_pair_boundary() -> Result<(), String> {
    let (database, id, data) = counted_database(true)?;
    assert_eq!(cached_font_has_char(&database, id, 'A'), Some(true));
    assert_eq!(
        cached_font_has_char(&database, id, '\u{10ffff}'),
        Some(false)
    );
    let characters = ('\u{1000}'..='\u{2000}').collect::<Vec<_>>();
    for ch in &characters {
        assert!(cached_font_has_char(&database, id, *ch).is_some());
    }
    let reads = data.reads.load(Ordering::Relaxed);
    assert_eq!(cached_font_has_char(&database, id, 'A'), Some(true));
    assert_eq!(
        cached_font_has_char(&database, id, '\u{10ffff}'),
        Some(false)
    );
    for ch in characters {
        assert!(cached_font_has_char(&database, id, ch).is_some());
    }
    assert_eq!(data.reads.load(Ordering::Relaxed), reads);
    Ok(())
}

#[test]
fn html_runs_reuse_glyph_support_across_style_keys_and_keep_unsupported_base() -> Result<(), String>
{
    use super::super::super::fallback::html_font_runs;
    let (database, id, data) = counted_database(true)?;
    for text in ["A", "\u{10ffff}"] {
        assert_eq!(
            html_font_runs(&database, id, text, 400, false),
            [(id, text.into())]
        );
        let reads = data.reads.load(Ordering::Relaxed);
        assert_eq!(
            html_font_runs(&database, id, text, 700, true),
            [(id, text.into())]
        );
        assert_eq!(data.reads.load(Ordering::Relaxed), reads);
    }
    Ok(())
}

#[test]
fn invalid_font_and_missing_face_are_not_memoized() -> Result<(), String> {
    let (database, id, data) = counted_database(false)?;
    assert_eq!(cached_font_has_char(&database, id, 'A'), None);
    let reads = data.reads.load(Ordering::Relaxed);
    assert_eq!(cached_font_has_char(&database, id, 'A'), None);
    assert_eq!(data.reads.load(Ordering::Relaxed), reads + 1);
    assert_eq!(
        cached_font_has_char(&Arc::new(Database::new()), ID::default(), 'A'),
        None
    );
    Ok(())
}

#[test]
fn database_identity_and_copy_on_write_do_not_reuse_another_faces_support() -> Result<(), String> {
    let (mut valid, id, _) = counted_database(true)?;
    let (invalid, other_id, _) = counted_database(false)?;
    assert_eq!(id, other_id);
    assert_eq!(cached_font_has_char(&valid, id, 'A'), Some(true));
    assert_eq!(cached_font_has_char(&invalid, other_id, 'A'), None);
    let previous = Arc::downgrade(&valid);
    Arc::make_mut(&mut valid).remove_face(id);
    assert!(previous.upgrade().is_none());
    assert_eq!(cached_font_has_char(&valid, id, 'A'), None);
    Ok(())
}

#[path = "svg_rasterize_text_glyph_cache_file_tests.rs"]
mod file_tests;

#[test]
fn expired_databases_are_removed_and_oldest_live_database_is_evicted() {
    let expired = Arc::new(Database::new());
    let mut entries = vec![GlyphCacheEntry {
        database: Arc::downgrade(&expired),
        glyphs: HashMap::new(),
        file_stamps: HashMap::new(),
    }];
    drop(expired);
    let databases = (0..=MAX_DATABASES)
        .map(|_| Arc::new(Database::new()))
        .collect::<Vec<_>>();
    let key = (ID::default(), 'A');
    assert_eq!(lookup(&mut entries, &databases[0], key, false, None), None);
    assert!(entries.is_empty());
    for database in &databases {
        insert(&mut entries, database, key, false, None);
    }
    assert_eq!(entries.len(), MAX_DATABASES);
    assert_eq!(lookup(&mut entries, &databases[0], key, false, None), None);
    assert_eq!(
        lookup(&mut entries, &databases[1], key, false, None),
        Some(false)
    );
}

#[test]
fn glyph_capacity_evicts_one_pair_and_keeps_valid_negative_values() -> Result<(), String> {
    let database = Arc::new(Database::new());
    let mut entries = vec![GlyphCacheEntry {
        database: Weak::new(),
        glyphs: HashMap::new(),
        file_stamps: HashMap::new(),
    }];
    for value in 0..=MAX_GLYPHS {
        let ch = char::from_u32(0x10000 + value as u32).ok_or("invalid fixture character")?;
        insert(&mut entries, &database, (ID::default(), ch), false, None);
    }
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].glyphs.len(), MAX_GLYPHS);
    assert_eq!(
        lookup(
            &mut entries,
            &database,
            (ID::default(), '\u{20000}'),
            false,
            None
        ),
        Some(false)
    );
    Ok(())
}

fn fill_database_cache(
    entries: &mut Vec<GlyphCacheEntry>,
    database: &Arc<Database>,
    characters: std::ops::RangeInclusive<char>,
) {
    for ch in characters {
        insert(entries, database, (ID::default(), ch), false, None);
    }
}

#[test]
fn eight_full_databases_keep_hash_table_allocation_bounded_after_churn() {
    let databases = (0..MAX_DATABASES)
        .map(|_| Arc::new(Database::new()))
        .collect::<Vec<_>>();
    let mut entries = Vec::new();
    for database in &databases {
        fill_database_cache(&mut entries, database, '\u{10000}'..='\u{1ffff}');
        fill_database_cache(&mut entries, database, '\u{20000}'..='\u{2000f}');
    }
    assert_eq!(entries.len(), MAX_DATABASES);
    assert!(entries.iter().all(|entry| entry.glyphs.len() == MAX_GLYPHS));
    let capacities = entries
        .iter()
        .map(|entry| entry.glyphs.capacity())
        .collect::<Vec<_>>();
    assert_allocation_bounds(&capacities);
}

fn assert_allocation_bounds(capacities: &[usize]) {
    let bucket_bytes = std::mem::size_of::<(GlyphKey, bool)>();
    let glyph_allocation_bound = glyph_table_allocation_bound(capacities, bucket_bytes);
    let stamp_bucket_bytes = std::mem::size_of::<(ID, FileStamp)>();
    let stamp_allocation_bound = MAX_DATABASES
        * ((MAX_FILE_STAMPS * 2) * (stamp_bucket_bytes + 1) + HASH_TABLE_CONTROL_GROUP_BYTES);
    let allocation_bound = glyph_allocation_bound + stamp_allocation_bound;
    eprintln!(
        "glyph cache: key={} bucket={bucket_bytes} capacities={capacities:?} glyph_bound={glyph_allocation_bound} stamp_bound={stamp_allocation_bound} total_bound={allocation_bound}",
        std::mem::size_of::<GlyphKey>()
    );
    assert!(bucket_bytes <= 24);
    assert!(stamp_bucket_bytes <= 128);
    assert!(allocation_bound < 64 * 1024 * 1024);
    assert!(
        capacities
            .iter()
            .all(|capacity| *capacity <= MAX_GLYPHS * 4)
    );
}

const HASH_TABLE_CONTROL_GROUP_BYTES: usize = 16;

fn glyph_table_allocation_bound(capacities: &[usize], bucket_bytes: usize) -> usize {
    capacities
        .iter()
        .map(|capacity| {
            capacity.next_power_of_two() * (bucket_bytes + 1) + HASH_TABLE_CONTROL_GROUP_BYTES
        })
        .sum()
}
