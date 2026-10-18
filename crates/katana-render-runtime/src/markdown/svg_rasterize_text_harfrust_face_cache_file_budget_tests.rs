use super::*;

#[test]
fn two_large_real_file_faces_evict_before_exceeding_single_source_budget()
-> Result<(), Box<dyn std::error::Error>> {
    let paths = [
        super::temp_font_path("budget-a"),
        super::temp_font_path("budget-b"),
    ];
    let result = check_large_real_files(&paths);
    for path in paths {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    result
}
fn check_large_real_files(
    paths: &[std::path::PathBuf; 2],
) -> Result<(), Box<dyn std::error::Error>> {
    let length = (super::MAX_SOURCE_BYTES / 2 + 1) as u64;
    let scope = FontFaceCacheScope::enter();
    for path in paths {
        std::fs::write(path, super::FONT_BYTES)?;
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)?
            .set_len(length)?;
        let (database, id) = super::file_database(path)?;
        let font = font_for_face(&database, id).ok_or("large real face missing")?;
        shape_and_commit(&database, id, &font, "large real face").map_err(std::io::Error::other)?;
        super::super::ACTIVE_SCOPE.with(|cache| {
            let cache = cache.borrow();
            assert!(cache.spare.is_none());
            let retained = cache
                .entry
                .iter()
                .map(|entry| entry.font.retained_source_bytes)
                .sum::<usize>();
            assert!(retained <= super::MAX_SOURCE_BYTES);
        });
    }
    drop(scope);
    assert_all_handles_cleared();
    Ok(())
}
#[test]
fn three_real_lazy_table_backings_admit_before_reading_the_next_font()
-> Result<(), Box<dyn std::error::Error>> {
    let paths = [
        super::temp_font_path("peak-a"),
        super::temp_font_path("peak-b"),
        super::temp_font_path("peak-c"),
    ];
    let result = check_three_backed_fonts(&paths);
    for path in paths {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    result
}
fn write_backed_hhea(path: &Path, table_length: u32) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = super::FONT_BYTES.to_vec();
    let table_count = u16::from_be_bytes([
        bytes[super::SFNT_NUM_TABLES_FIELD_OFFSET],
        bytes[super::SFNT_NUM_TABLES_FIELD_OFFSET + 1],
    ]) as usize;
    let offset = (0..table_count)
        .map(|index| super::SFNT_TABLE_DIRECTORY_START + index * super::SFNT_TABLE_RECORD_SIZE)
        .find(|index| &bytes[*index..*index + super::SFNT_TAG_BYTE_LENGTH] == b"hhea")
        .ok_or("real hhea record missing")?;
    let table_offset = u32::from_be_bytes(
        bytes[offset + super::SFNT_TABLE_OFFSET_FIELD_START
            ..offset + super::SFNT_TABLE_OFFSET_FIELD_END]
            .try_into()?,
    );
    bytes[offset + super::SFNT_TABLE_LENGTH_FIELD_START
        ..offset + super::SFNT_TABLE_LENGTH_FIELD_END]
        .copy_from_slice(&table_length.to_be_bytes());
    std::fs::write(path, bytes)?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_len(u64::from(table_offset) + u64::from(table_length))?;
    Ok(())
}
fn check_three_backed_fonts(
    paths: &[std::path::PathBuf; super::BACKED_FONT_COUNT],
) -> Result<(), Box<dyn std::error::Error>> {
    let scope = FontFaceCacheScope::enter();
    for (index, path) in paths.iter().enumerate() {
        check_backed_font(path, index)?;
    }
    drop(scope);
    assert_all_handles_cleared();
    Ok(())
}
fn check_backed_font(
    path: &std::path::Path,
    index: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let table_length = if index < super::FIRST_BACKED_FONT_COUNT {
        super::FIRST_BACKED_FONT_BYTES as u32
    } else {
        super::NEXT_BACKED_FONT_BYTES as u32
    };
    write_backed_hhea(path, table_length)?;
    let (database, id) = super::file_database(path)?;
    let durable = font_source_generation(&database, id).durable_reusable();
    let font = font_for_face(&database, id).ok_or("backed lazy face missing")?;
    if durable {
        assert_backed_font_budget(index, font.retained_source_bytes);
    }
    assert_backed_font_table(&font, table_length)?;
    assert!(super::super::commit_file_font(&database, id, &font));
    drop(font);
    if durable && index == 1 {
        assert_two_backed_fonts_retained();
    }
    Ok(())
}
fn assert_backed_font_table(
    font: &FontHandle,
    table_length: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    assert!(font.metrics().h_line().is_some());
    assert_eq!(
        font.tables()
            .hhea_data()
            .ok_or("backed hhea missing")?
            .len(),
        table_length as usize
    );
    Ok(())
}
fn assert_backed_font_budget(index: usize, next_bytes: usize) {
    super::super::ACTIVE_SCOPE.with(|cache| {
        let cache = cache.borrow();
        let retained = cache
            .entry
            .iter()
            .chain(cache.spare.iter())
            .map(|entry| entry.font.retained_source_bytes)
            .sum::<usize>();
        /* WHY: commit前にも次handleを含むbacking予算が上限内である。 */
        assert!(retained + next_bytes <= super::MAX_SOURCE_BYTES);
        if index == 2 {
            assert!(cache.entry.is_some() && cache.spare.is_none());
        }
    });
}
fn assert_two_backed_fonts_retained() {
    super::super::ACTIVE_SCOPE.with(|cache| {
        let cache = cache.borrow();
        assert!(cache.entry.is_some() && cache.spare.is_some());
    });
}
