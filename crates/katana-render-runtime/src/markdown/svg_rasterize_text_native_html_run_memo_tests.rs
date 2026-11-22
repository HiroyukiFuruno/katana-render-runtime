use super::super::super::font::platform_fallback::actual_fallback::NativeHtmlFallbackBase;
use super::super::SelectionQuery;
use super::super::test_support::{FONT_SIZE, REGULAR_WEIGHT, roboto_flex};
use super::{
    Arc, FileFace, HashMap, MAX_GLYPH_QUERIES, MAX_PHYSICAL_FONTS, MAX_STYLE_QUERIES, Memo,
    SelectionKey, actual_face_for, native_html_fallback, observe_path_stamp, selected_face,
    verified_database,
};
use resvg::usvg::fontdb::{Database, ID};
use std::error::Error;

fn system_selection() -> Result<super::NativeHtmlFallback, Box<dyn Error>> {
    Ok(native_html_fallback(
        NativeHtmlFallbackBase::SystemUi,
        '装',
        f64::from(FONT_SIZE),
        REGULAR_WEIGHT,
        Some("zh-Hans"),
    )
    .ok_or("native system fallback unavailable")?)
}

#[test]
fn native_face_cache_rejects_new_entries_at_the_physical_face_limit() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let fixture = roboto_flex(&directory)?;
    let stamp = observe_path_stamp(&fixture.path)
        .ok()
        .ok_or("fixture stamp unavailable")?;
    let database = Arc::new(fixture.database.clone());
    let mut memo = Memo {
        glyphs: HashMap::new(),
        faces: (0..MAX_PHYSICAL_FONTS)
            .map(|_| FileFace {
                path: fixture.path.clone(),
                name: "Roboto Flex".into(),
                stamp: stamp.clone(),
                database: Arc::clone(&database),
                id: fixture.face_id,
            })
            .collect(),
    };
    let selection = system_selection()?;
    assert!(memo.load_face(&selection.font, stamp).is_none());
    Ok(())
}

#[test]
fn verified_database_rejects_a_real_face_when_its_observed_file_stamp_differs()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = roboto_flex(&directory)?;
    let unrelated_stamp = observe_path_stamp(&fixture.path)
        .ok()
        .ok_or("fixture stamp unavailable")?;
    let selection = system_selection()?;
    let actual = actual_face_for(&selection.font).ok_or("actual face unavailable")?;
    assert!(verified_database(&actual, &unrelated_stamp).is_none());
    Ok(())
}

fn system_query(database: &Database) -> SelectionQuery<'_> {
    SelectionQuery {
        database,
        base: ID::dummy(),
        system_ui: true,
        key: SelectionKey {
            base: "System Font".into(),
            size: FONT_SIZE.to_bits(),
            weight: REGULAR_WEIGHT,
            language: Some("zh-Hans".into()),
        },
    }
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
    let glyphs: HashMap<_, _> = ('\u{e000}'..='\u{f8ff}')
        .take(MAX_GLYPH_QUERIES)
        .map(|ch| (ch, None))
        .collect();
    assert_eq!(glyphs.len(), MAX_GLYPH_QUERIES);
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
    let parsed_without_glyph = fixture
        .database
        .with_face_data(fixture.face_id, |bytes, index| {
            rustybuzz::ttf_parser::Face::parse(bytes, index)
                .ok()
                .is_some_and(|face| face.glyph_index('装').is_none())
        });
    assert_eq!(parsed_without_glyph, Some(true));
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
