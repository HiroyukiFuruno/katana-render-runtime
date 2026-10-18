use super::*;

#[cfg(unix)]
#[test]
fn populated_spare_file_mutation_rejects_and_retries_without_old_handles()
-> Result<(), Box<dyn std::error::Error>> {
    let paths = [
        super::temp_font_path("spare-mutation-a"),
        super::temp_font_path("spare-mutation-b"),
    ];
    let result = check_spare_mutation(&paths);
    for path in paths {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    result
}
#[cfg(unix)]
fn check_spare_mutation(paths: &[std::path::PathBuf; 2]) -> Result<(), Box<dyn std::error::Error>> {
    for path in paths {
        std::fs::write(path, super::FONT_BYTES)?;
    }
    let (first_db, first_id) = super::file_database(&paths[0])?;
    let (second_db, second_id) = super::file_database(&paths[1])?;
    if !font_source_generation(&first_db, first_id).durable_reusable()
        || !font_source_generation(&second_db, second_id).durable_reusable()
    {
        return check_nondurable_file_pair(&first_db, first_id, &second_db, second_id);
    }
    check_durable_spare_mutation(paths, &first_db, first_id, &second_db, second_id)
}
#[cfg(unix)]
fn check_nondurable_file_pair(
    first_db: &Database,
    first_id: resvg::usvg::fontdb::ID,
    second_db: &Database,
    second_id: resvg::usvg::fontdb::ID,
) -> Result<(), Box<dyn std::error::Error>> {
    let scope = FontFaceCacheScope::enter();
    for (database, id) in [(first_db, first_id), (second_db, second_id)] {
        let font = font_for_face(database, id).ok_or("nondurable face missing")?;
        shape_and_commit(database, id, &font, "nondurable").map_err(std::io::Error::other)?;
        assert_all_handles_cleared();
    }
    drop(scope);
    assert_all_handles_cleared();
    Ok(())
}
#[cfg(unix)]
fn check_durable_spare_mutation(
    paths: &[std::path::PathBuf; 2],
    first_db: &Database,
    first_id: resvg::usvg::fontdb::ID,
    second_db: &Database,
    second_id: resvg::usvg::fontdb::ID,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut attempts = 0;
    let result = super::super::super::super::font::with_validated_stamp_batch(|| {
        attempts += 1;
        spare_mutation_attempt(paths, first_db, first_id, second_db, second_id, attempts)
    });
    result?;
    assert_eq!(attempts, 2);
    Ok(())
}
#[cfg(unix)]
fn spare_mutation_attempt(
    paths: &[std::path::PathBuf; 2],
    first_db: &Database,
    first_id: resvg::usvg::fontdb::ID,
    second_db: &Database,
    second_id: resvg::usvg::fontdb::ID,
    attempt: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let scope = FontFaceCacheScope::enter();
    let first = font_for_face(first_db, first_id).ok_or("mutation first face missing")?;
    shape_and_commit(first_db, first_id, &first, "mutation first")
        .map_err(std::io::Error::other)?;
    let second = font_for_face(second_db, second_id).ok_or("mutation second face missing")?;
    shape_and_commit(second_db, second_id, &second, "mutation second")
        .map_err(std::io::Error::other)?;
    if attempt == 1 {
        mutate_populated_spare(paths, first_db, first_id, &first)?;
    } else {
        commit_fresh_mutation_face(first_db, first_id)?;
    }
    drop(scope);
    assert_all_handles_cleared();
    Ok(())
}
#[cfg(unix)]
fn mutate_populated_spare(
    paths: &[std::path::PathBuf; 2],
    first_db: &Database,
    first_id: resvg::usvg::fontdb::ID,
    first: &super::super::FontHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    super::super::ACTIVE_SCOPE.with(|cache| {
        let cache = cache.borrow();
        assert!(cache.entry.is_some() && cache.spare.is_some());
        assert_eq!(
            cache.spare.as_ref().map(|entry| &entry.key),
            first.key.as_ref()
        );
    });
    std::fs::write(&paths[0], [super::FONT_BYTES, &[0_u8][..]].concat())?;
    assert!(font_for_face(first_db, first_id).is_none());
    assert_all_handles_cleared();
    assert!(!super::super::commit_file_font(first_db, first_id, first));
    assert_all_handles_cleared();
    Ok(())
}
#[cfg(unix)]
fn commit_fresh_mutation_face(
    database: &Database,
    id: resvg::usvg::fontdb::ID,
) -> Result<(), Box<dyn std::error::Error>> {
    let fresh = font_for_face(database, id).ok_or("fresh mutation face missing")?;
    assert!(fresh.metrics().h_line().is_some());
    assert!(super::super::commit_file_font(database, id, &fresh));
    Ok(())
}
