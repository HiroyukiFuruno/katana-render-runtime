use super::super::super::font::platform_fallback::actual_fallback::{
    NativeHtmlFallbackBase, actual_face_for,
};
use super::super::SelectionQuery;
use super::super::test_support::{FONT_SIZE, REGULAR_WEIGHT, roboto_flex};
use super::files::{FileFace, name_table_digest, verified_database};
use super::{
    Arc, HashMap, MAX_GLYPH_QUERIES, MAX_PHYSICAL_FONTS, MAX_STYLE_QUERIES, Memo, SelectionKey,
    actual_font_metadata_for, native_html_fallback, observe_path_stamp, selected_face,
    with_current,
};
use crate::markdown::svg_rasterize::HtmlFontFrameCache;
use resvg::usvg::fontdb::{Database, ID};
use std::error::Error;

fn system_selection() -> Result<super::NativeHtmlFallback, Box<dyn Error>> {
    native_html_fallback(
        NativeHtmlFallbackBase::SystemUi,
        '装',
        f64::from(FONT_SIZE),
        REGULAR_WEIGHT,
        Some("zh-Hans"),
    )
    .ok_or_else(|| "native system fallback unavailable".into())
}

fn selection_key() -> SelectionKey {
    SelectionKey {
        base: "System Font".into(),
        size: FONT_SIZE.to_bits(),
        weight: REGULAR_WEIGHT,
        language: Some("zh-Hans".into()),
    }
}

fn stored_face() -> Result<(Memo, FileFace), Box<dyn Error>> {
    let selection = system_selection()?;
    let mut memo = Memo::default();
    memo.physical_face(&selection.font)
        .ok_or("physical face missing")?;
    let face = memo.faces.first().ok_or("face was not stored")?.clone();
    Ok((memo, face))
}

fn frame_face(
    cache: &HtmlFontFrameCache,
    mark_glyph: bool,
) -> Result<(Arc<Database>, ID, bool), Box<dyn Error>> {
    let selection = system_selection()?;
    cache.with_frame(|| {
        with_current(|memo| {
            let empty = memo.glyphs.is_empty();
            let face = memo
                .physical_face(&selection.font)
                .ok_or("physical face missing")?;
            if mark_glyph {
                memo.glyphs.insert(selection_key(), HashMap::new());
            }
            Ok((face.0, face.1, empty))
        })
        .ok_or("native frame memo missing")?
    })
}

fn system_query(database: &Database) -> SelectionQuery<'_> {
    SelectionQuery {
        database,
        base: ID::dummy(),
        system_ui: true,
        key: selection_key(),
    }
}

#[test]
fn native_face_cache_rejects_new_entries_at_the_physical_face_limit() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let fixture = roboto_flex(&directory)?;
    let stamp = observe_path_stamp(&fixture.path).map_err(|()| "fixture stamp unavailable")?;
    let database = Arc::new(fixture.database.clone());
    let mut memo = Memo {
        faces: (0..MAX_PHYSICAL_FONTS)
            .map(|_| FileFace {
                path: fixture.path.clone(),
                name: "Roboto Flex".into(),
                stamp: stamp.clone(),
                database: Arc::clone(&database),
                id: fixture.face_id,
                name_table: None,
            })
            .collect(),
        ..Memo::default()
    };
    let selection = system_selection()?;
    let metadata = actual_font_metadata_for(&selection.font).ok_or("physical metadata missing")?;
    assert!(memo.load_face(&selection.font, &metadata, stamp).is_none());
    Ok(())
}

#[test]
fn physical_font_database_reuses_only_session_cache_between_frames() -> Result<(), Box<dyn Error>> {
    let cache = HtmlFontFrameCache::default();
    let first = frame_face(&cache, true)?;
    let second = frame_face(&cache, false)?;
    let fresh = frame_face(&HtmlFontFrameCache::default(), false)?;
    assert!(first.2 && second.2, "frame glyph memo must reset");
    assert!(Arc::ptr_eq(&first.0, &second.0));
    assert_eq!(first.1, second.1);
    assert!(!Arc::ptr_eq(&first.0, &fresh.0));
    Ok(())
}

#[test]
fn physical_font_cache_rejects_changed_file_stamp_and_name_table() -> Result<(), Box<dyn Error>> {
    let (memo, _) = stored_face()?;
    let selection = system_selection()?;
    let metadata = actual_font_metadata_for(&selection.font).ok_or("physical metadata missing")?;
    let stamp = observe_path_stamp(&metadata.canonical_path).map_err(|()| "font stamp missing")?;
    let name_table = name_table_digest(&selection.font).ok_or("name table missing")?;
    let directory = tempfile::tempdir()?;
    let other = roboto_flex(&directory)?;
    let changed = observe_path_stamp(&other.path).map_err(|()| "changed stamp missing")?;
    let mut different_name_table = name_table;
    different_name_table[0] ^= 1;
    assert!(memo.files.find(&metadata, &changed, name_table).is_none());
    assert!(
        memo.files
            .find(&metadata, &stamp, different_name_table)
            .is_none()
    );
    Ok(())
}

#[test]
fn physical_font_cache_replaces_old_identities_and_evicts_at_its_limit()
-> Result<(), Box<dyn Error>> {
    let (memo, original) = stored_face()?;
    for index in 0..MAX_PHYSICAL_FONTS {
        memo.files.store(FileFace {
            name: format!("face {index}"),
            ..original.clone()
        });
    }
    let faces = memo.files.0.borrow();
    assert_eq!(faces.len(), MAX_PHYSICAL_FONTS);
    assert!(faces.iter().all(|face| face.name != original.name));
    drop(faces);
    memo.files.store(FileFace {
        name: "face 0".into(),
        ..original
    });
    let replacements = memo.files.0.borrow();
    assert_eq!(
        replacements
            .iter()
            .filter(|face| face.name == "face 0")
            .count(),
        1
    );
    Ok(())
}

#[test]
fn verified_database_rejects_a_real_face_when_its_observed_file_stamp_differs()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = roboto_flex(&directory)?;
    let stamp = observe_path_stamp(&fixture.path).map_err(|()| "fixture stamp unavailable")?;
    let selection = system_selection()?;
    let actual = actual_face_for(&selection.font).ok_or("actual face unavailable")?;
    assert!(verified_database(&actual, &stamp).is_none());
    Ok(())
}

#[test]
fn selection_does_not_cache_a_new_style_after_the_style_limit() {
    let database = Database::new();
    let query = system_query(&database);
    let mut memo = Memo::default();
    for index in 0..MAX_STYLE_QUERIES {
        memo.glyphs.insert(
            SelectionKey {
                base: index.to_string(),
                ..query.key.clone()
            },
            HashMap::new(),
        );
    }
    let _ = memo.selection(&query, '装');
    assert!(!memo.glyphs.contains_key(&query.key));
}

#[test]
fn selection_does_not_cache_a_glyph_after_the_glyph_limit() {
    let database = Database::new();
    let query = system_query(&database);
    let mut memo = Memo::default();
    let glyphs = ('\u{e000}'..='\u{f8ff}')
        .take(MAX_GLYPH_QUERIES)
        .map(|ch| (ch, None))
        .collect();
    memo.glyphs.insert(query.key.clone(), glyphs);
    let _ = memo.selection(&query, '装');
    assert!(!memo.glyphs[&query.key].contains_key(&'装'));
}

#[test]
fn selected_face_rejects_physical_axes_for_a_missing_roboto_flex_glyph()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = roboto_flex(&directory)?;
    let selection = system_selection()?;
    let missing = fixture
        .database
        .with_face_data(fixture.face_id, |bytes, index| {
            rustybuzz::ttf_parser::Face::parse(bytes, index)
                .ok()
                .is_some_and(|face| face.glyph_index('装').is_none())
        });
    assert_eq!(missing, Some(true));
    assert!(
        selected_face(
            Arc::new(fixture.database.clone()),
            fixture.face_id,
            selection,
            '装'
        )
        .is_none()
    );
    Ok(())
}
