use super::super::super::font::platform_fallback::actual_fallback::NativeHtmlFallbackBase;
use super::super::test_support::{FONT_SIZE, REGULAR_WEIGHT, roboto_flex};
use super::{
    Arc, FileFace, HashMap, MAX_PHYSICAL_FONTS, Memo, actual_face_for, native_html_fallback,
    observe_path_stamp, verified_database,
};
fn system_selection() -> Result<super::NativeHtmlFallback, String> {
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
fn native_face_cache_rejects_new_entries_at_the_physical_face_limit() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = roboto_flex(&directory)?;
    let stamp = observe_path_stamp(&fixture.path).map_err(|_| "fixture stamp unavailable")?;
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
fn verified_database_rejects_a_real_face_when_its_observed_file_stamp_differs() -> Result<(), String>
{
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = roboto_flex(&directory)?;
    let unrelated_stamp =
        observe_path_stamp(&fixture.path).map_err(|_| "fixture stamp unavailable")?;
    let selection = system_selection()?;
    let actual = actual_face_for(&selection.font).ok_or("actual face unavailable")?;
    assert!(verified_database(&actual, &unrelated_stamp).is_none());
    Ok(())
}
