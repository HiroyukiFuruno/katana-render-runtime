use super::*;

#[test]
fn alternating_real_file_faces_share_library_metrics_and_keep_bounded_sources()
-> Result<(), Box<dyn std::error::Error>> {
    let paths = [
        super::temp_font_path("two-face-a"),
        super::temp_font_path("two-face-b"),
    ];
    let result = check_alternating_real_files(&paths);
    for path in paths {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    result
}
fn check_alternating_real_files(
    paths: &[std::path::PathBuf; 2],
) -> Result<(), Box<dyn std::error::Error>> {
    for path in paths {
        std::fs::write(path, super::FONT_BYTES)?;
    }
    let (first_db, first_id) = super::file_database(&paths[0])?;
    let (second_db, second_id) = super::file_database(&paths[1])?;
    check_cached_file_pair(&first_db, first_id, &second_db, second_id)?;
    check_repopulated_file_pair(&first_db, first_id, &second_db, second_id)?;
    check_malformed_file_fallback(&first_db, first_id)
}
fn check_cached_file_pair(
    first_db: &Database,
    first_id: resvg::usvg::fontdb::ID,
    second_db: &Database,
    second_id: resvg::usvg::fontdb::ID,
) -> Result<(), Box<dyn std::error::Error>> {
    let scope = FontFaceCacheScope::enter();
    let first = font_for_face(first_db, first_id).ok_or("first real face missing")?;
    shape_and_commit(first_db, first_id, &first, "first").map_err(std::io::Error::other)?;
    let second = font_for_face(second_db, second_id).ok_or("second real face missing")?;
    shape_and_commit(second_db, second_id, &second, "second").map_err(std::io::Error::other)?;
    if font_source_generation(first_db, first_id).durable_reusable()
        && font_source_generation(second_db, second_id).durable_reusable()
    {
        assert_reused_file_handle(first_db, first_id, &first)?;
        assert_reused_file_handle(second_db, second_id, &second)?;
        assert_two_file_cache_budget(Some(super::FONT_BYTES.len() * 2));
    }
    drop(scope);
    assert_all_handles_cleared();
    Ok(())
}
fn assert_reused_file_handle(
    database: &Database,
    id: resvg::usvg::fontdb::ID,
    held: &super::super::FontHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let repeated = font_for_face(database, id).ok_or("repeated real face missing")?;
    assert!(std::ptr::eq(held.metrics(), repeated.metrics()));
    assert_eq!(held.metrics(), repeated.metrics());
    assert_eq!(
        super::shape_width(held, 18.25, "AVoffice")?,
        super::shape_width(&repeated, 18.25, "AVoffice")?
    );
    assert!(super::super::commit_file_font(database, id, &repeated));
    Ok(())
}
fn assert_two_file_cache_budget(expected_retained_bytes: Option<usize>) {
    super::super::ACTIVE_SCOPE.with(|cache| {
        let cache = cache.borrow();
        assert!(cache.entry.is_some() && cache.spare.is_some());
        let retained = cache
            .entry
            .iter()
            .chain(cache.spare.iter())
            .map(|entry| entry.font.retained_source_bytes)
            .sum::<usize>();
        if let Some(expected) = expected_retained_bytes {
            assert_eq!(retained, expected);
        }
        assert!(retained <= super::MAX_SOURCE_BYTES);
    });
}
fn check_repopulated_file_pair(
    first_db: &Database,
    first_id: resvg::usvg::fontdb::ID,
    second_db: &Database,
    second_id: resvg::usvg::fontdb::ID,
) -> Result<(), Box<dyn std::error::Error>> {
    let scope = FontFaceCacheScope::enter();
    for (database, id) in [(&first_db, first_id), (&second_db, second_id)] {
        let font = font_for_face(database, id).ok_or("repopulated real face missing")?;
        shape_and_commit(database, id, &font, "repopulated").map_err(std::io::Error::other)?;
    }
    drop(scope);
    assert_all_handles_cleared();
    Ok(())
}
fn check_malformed_file_fallback(
    first_db: &Database,
    first_id: resvg::usvg::fontdb::ID,
) -> Result<(), Box<dyn std::error::Error>> {
    let scope = FontFaceCacheScope::enter();
    /* WHY: malformed binaryへの切替は、2つの正常library handleも残さない。 */
    let mut info = first_db
        .face(first_id)
        .ok_or("first face info missing")?
        .clone();
    let mut malformed = super::FONT_BYTES.to_vec();
    malformed[0] = !malformed[0];
    info.source = Source::Binary(std::sync::Arc::new(malformed));
    let mut malformed_db = Database::new();
    malformed_db.push_face_info(info);
    let malformed_id = malformed_db
        .faces()
        .next()
        .ok_or("malformed face info missing")?
        .id;
    assert!(font_for_face(&malformed_db, malformed_id).is_none());
    assert_all_handles_cleared();
    drop(scope);
    assert_all_handles_cleared();
    Ok(())
}
#[cfg(target_os = "macos")]
#[test]
fn distinct_real_font_files_reuse_library_metrics_without_skipping()
-> Result<(), Box<dyn std::error::Error>> {
    let source = Path::new("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc");
    let paths = [
        super::temp_font_path("distinct-noto"),
        super::temp_font_path("distinct-hiragino"),
    ];
    let result = (|| {
        std::fs::write(&paths[0], super::FONT_BYTES)?;
        std::fs::copy(source, &paths[1])?;
        let (first_db, first_id) = super::file_database(&paths[0])?;
        let (second_db, second_id) = super::file_database(&paths[1])?;
        let first_face = first_db
            .face(first_id)
            .ok_or("first face metadata missing")?;
        let second_face = second_db
            .face(second_id)
            .ok_or("second face metadata missing")?;
        assert_eq!((first_face.index, second_face.index), (0, 0));
        assert_ne!(first_face.post_script_name, second_face.post_script_name);
        assert_ne!(std::fs::read(&paths[0])?, std::fs::read(&paths[1])?);
        assert_distinct_alternating_reuse((&first_db, first_id), (&second_db, second_id))
    })();
    for path in paths {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    result
}
#[cfg(target_os = "macos")]
#[test]
fn distinct_real_ttc_indices_reuse_library_metrics_without_skipping()
-> Result<(), Box<dyn std::error::Error>> {
    let source = Path::new("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc");
    let path = super::temp_font_path("distinct-ttc-indices");
    let result = (|| {
        std::fs::copy(source, &path)?;
        let mut database = Database::new();
        database.load_font_source(Source::File(path.clone()));
        check_ttc_indices(&database)
    })();
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    result
}
#[cfg(target_os = "macos")]
fn check_ttc_indices(database: &Database) -> Result<(), Box<dyn std::error::Error>> {
    let first_id = ttc_face_id(database, 0).ok_or("required real TTC index 0 missing")?;
    let second_id = ttc_face_id(database, 2).ok_or("required real TTC index 2 missing")?;
    assert_ne!(first_id, second_id);
    assert_eq!(
        database
            .face(first_id)
            .ok_or("TTC first face missing")?
            .index,
        0
    );
    assert_eq!(
        database
            .face(second_id)
            .ok_or("TTC second face missing")?
            .index,
        2
    );
    assert_distinct_alternating_reuse((database, first_id), (database, second_id))
}
#[cfg(target_os = "macos")]
fn ttc_face_id(database: &Database, index: u32) -> Option<resvg::usvg::fontdb::ID> {
    database
        .faces()
        .find(|face| face.index == index)
        .map(|face| face.id)
}
#[cfg(target_os = "macos")]
fn assert_distinct_alternating_reuse(
    first: (&Database, resvg::usvg::fontdb::ID),
    second: (&Database, resvg::usvg::fontdb::ID),
) -> Result<(), Box<dyn std::error::Error>> {
    assert!(font_source_generation(first.0, first.1).durable_reusable());
    assert!(font_source_generation(second.0, second.1).durable_reusable());
    let scope = FontFaceCacheScope::enter();
    let held_first = font_for_face(first.0, first.1).ok_or("distinct first face missing")?;
    shape_and_commit(first.0, first.1, &held_first, "distinct first")
        .map_err(std::io::Error::other)?;
    let held_second = font_for_face(second.0, second.1).ok_or("distinct second face missing")?;
    shape_and_commit(second.0, second.1, &held_second, "distinct second")
        .map_err(std::io::Error::other)?;
    assert!(!std::ptr::eq(held_first.metrics(), held_second.metrics()));
    for ((database, id), held) in [(first, &held_first), (second, &held_second)] {
        assert_face_reused_against_owned(database, id, held)?;
    }
    assert_two_file_cache_budget(None);
    drop(held_first);
    drop(held_second);
    drop(scope);
    assert_all_handles_cleared();
    Ok(())
}
#[cfg(target_os = "macos")]
fn assert_face_reused_against_owned(
    database: &Database,
    id: resvg::usvg::fontdb::ID,
    held: &super::super::FontHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let repeated = font_for_face(database, id).ok_or("distinct repeated face missing")?;
    assert!(std::ptr::eq(held.metrics(), repeated.metrics()));
    assert_eq!(held.metrics(), repeated.metrics());
    assert_eq!(
        super::shape_width(held, 18.25, "AVoffice")?,
        super::shape_width(&repeated, 18.25, "AVoffice")?
    );
    assert_owned_metrics_match(database, id, &repeated)?;
    assert!(super::super::commit_file_font(database, id, &repeated));
    drop(repeated);
    Ok(())
}
#[cfg(target_os = "macos")]
fn assert_owned_metrics_match(
    database: &Database,
    id: resvg::usvg::fontdb::ID,
    font: &super::super::FontHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let owned = database
        .with_face_data(id, |data, index| {
            harfrust::font::Font::new(data.to_vec(), index)
        })
        .flatten()
        .ok_or("distinct owned oracle missing")?;
    let index = database.face(id).ok_or("distinct metadata missing")?.index;
    super::file_ttc_tests::assert_lazy_matches_owned(font, &owned, index)?;
    Ok(())
}
