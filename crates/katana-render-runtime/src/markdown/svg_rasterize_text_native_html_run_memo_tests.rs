use super::super::super::font::platform_fallback::actual_fallback::{
    ActualCoreTextFace, NativeHtmlFallbackBase, actual_face_for,
};
use super::super::super::harfrust::MappedFontBytes;
use super::super::SelectionQuery;
use super::super::test_support::{FONT_SIZE, REGULAR_WEIGHT, roboto_flex};
use super::files::{FileFace, name_table_digest, verified_database};
use super::{
    Arc, HashMap, MAX_GLYPH_QUERIES, MAX_PHYSICAL_FONTS, MAX_STYLE_QUERIES, Memo, SelectionKey,
    actual_font_metadata_for, native_html_fallback, observe_path_stamp, selected_face,
    with_current,
};
use crate::markdown::svg_rasterize::HtmlFontFrameCache;
use crate::markdown::svg_rasterize::font_file_fixture::FontFileFixture;
use resvg::usvg::fontdb::{Database, ID, Source};
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

struct FrameFontState {
    database: Arc<Database>,
    face_id: ID,
    glyphs_were_empty: bool,
}

fn frame_face(
    cache: &HtmlFontFrameCache,
    mark_glyph: bool,
) -> Result<FrameFontState, Box<dyn Error>> {
    let selection = system_selection()?;
    cache.with_frame(|| {
        with_current(|memo| {
            let empty = memo.glyphs.is_empty();
            let face = memo
                .physical_face(&selection.font)
                .ok_or("physical face missing")?;
            let frame = FrameFontState {
                database: Arc::clone(&face.database),
                face_id: face.id,
                glyphs_were_empty: empty,
            };
            if mark_glyph {
                memo.glyphs.insert(selection_key(), HashMap::new());
            }
            Ok(frame)
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

fn fixture_face(fixture: &FontFileFixture) -> Result<FileFace, Box<dyn Error>> {
    Ok(FileFace {
        path: fixture.path.clone(),
        name: "Roboto Flex".into(),
        stamp: observe_path_stamp(&fixture.path).map_err(|()| "fixture stamp unavailable")?,
        database: Arc::new(fixture.database.clone()),
        id: fixture.face_id,
        name_table: None,
    })
}

#[test]
fn native_face_cache_rejects_new_entries_at_the_physical_face_limit() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let fixture = roboto_flex(&directory)?;
    let face = fixture_face(&fixture)?;
    let mut memo = Memo {
        faces: vec![face.clone(); MAX_PHYSICAL_FONTS],
        ..Memo::default()
    };
    let selection = system_selection()?;
    let metadata = actual_font_metadata_for(&selection.font).ok_or("physical metadata missing")?;
    assert!(
        memo.load_face(&selection.font, &metadata, face.stamp)
            .is_none()
    );
    Ok(())
}

#[test]
fn physical_font_database_reuses_only_session_cache_between_frames() -> Result<(), Box<dyn Error>> {
    let cache = HtmlFontFrameCache::default();
    let first = frame_face(&cache, true)?;
    let second = frame_face(&cache, false)?;
    let fresh = frame_face(&HtmlFontFrameCache::default(), false)?;
    assert!(first.glyphs_were_empty && second.glyphs_were_empty);
    assert!(Arc::ptr_eq(&first.database, &second.database));
    assert_eq!(first.face_id, second.face_id);
    assert!(!Arc::ptr_eq(&first.database, &fresh.database));
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
    assert!(verified_database(&actual, &stamp, None).is_none());
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
    let face = fixture_face(&fixture)?;
    assert!(selected_face(&face, selection, '装').is_none());
    Ok(())
}

#[test]
fn mapped_font_bytes_preserve_the_physical_face_index() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = roboto_flex(&directory)?;
    let face = fixture_face(&fixture)?;
    let physical = face.with_face_data(|bytes, index| (bytes.to_vec(), index));
    let bytes: MappedFontBytes = Arc::new(std::fs::read(&fixture.path)?);
    let font = fixture
        .database
        .face(fixture.face_id)
        .ok_or("fixture face missing")?;
    let actual = ActualCoreTextFace {
        canonical_path: fixture.path.clone(),
        face_index: font.index,
        post_script_name: font.post_script_name.clone(),
    };
    let (database, id) = verified_database(&actual, &face.stamp, Some(Arc::clone(&bytes)))
        .ok_or("verified font missing")?;
    assert!(physical.is_some());
    assert_eq!(
        physical,
        database.with_face_data(id, |bytes, index| (bytes.to_vec(), index))
    );
    assert!(
        matches!(database.face_source(id), Some((Source::SharedFile(path, shared), _))
        if path == fixture.path && Arc::ptr_eq(&bytes, &shared))
    );
    Ok(())
}
