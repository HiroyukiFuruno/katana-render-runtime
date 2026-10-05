use super::super::{FONT, binary_database, counted_options, first_face, select};
use super::*;
use resvg::usvg::fontdb::{Database, ID, Source};
use std::sync::atomic::{AtomicUsize, Ordering};

struct FontDirectory(std::path::PathBuf);

impl FontDirectory {
    fn create() -> TestResult<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "krr-selector-boundary-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn font_path(&self, index: usize) -> std::path::PathBuf {
        self.0.join(format!("font-{index}.ttf"))
    }
}

impl Drop for FontDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn database_with_257_file_paths(
    directory: &FontDirectory,
) -> TestResult<(std::path::PathBuf, Arc<Database>)> {
    let first_path = directory.font_path(0);
    std::fs::write(&first_path, FONT)?;
    let mut database = Database::new();
    database.load_font_source(Source::File(first_path.clone()));
    for index in 1..257 {
        let path = directory.font_path(index);
        std::fs::hard_link(&first_path, &path)?;
        database.load_font_source(Source::File(path));
    }
    Ok((first_path, Arc::new(database)))
}

fn selector_database_limit_bypasses_ninth_database() -> TestResult<()> {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut options = counted_options(Arc::clone(&calls));
    let databases: Vec<_> = (0..=super::super::super::entries::MAX_DATABASES)
        .map(|_| binary_database())
        .collect();
    super::super::super::super::with_validated_tree_parse(|| {
        for database in &databases {
            select(&mut options, database, 'x', &[]);
        }
        select(&mut options, &databases[0], 'x', &[]);
    });
    assert_eq!(calls.load(Ordering::Relaxed), databases.len());
    Ok(())
}

#[test]
fn database_limit_bypasses_only_new_database_and_preserves_existing_entry() -> TestResult<()> {
    selector_database_limit_bypasses_ninth_database()
}

#[test]
fn key_byte_limit_bypasses_oversized_keys_but_keeps_small_keys() -> TestResult<()> {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut options = counted_options(Arc::clone(&calls));
    let database = binary_database();
    let face = first_face(&database)?;
    let excluded =
        vec![face; super::super::super::entries::MAX_KEY_BYTES / std::mem::size_of::<ID>() + 1];
    super::super::super::super::with_validated_tree_parse(|| {
        select(&mut options, &database, 'x', &excluded);
        select(&mut options, &database, 'x', &excluded);
        select(&mut options, &database, 'y', &[]);
        select(&mut options, &database, 'y', &[]);
    });
    assert_eq!(calls.load(Ordering::Relaxed), 3);
    Ok(())
}

#[test]
fn two_hundred_fifty_seven_paths_retry_durable_parse_and_keep_weak_parse_uncached() -> TestResult<()>
{
    let directory = FontDirectory::create()?;
    let (first_path, database) = database_with_257_file_paths(&directory)?;
    assert_eq!(database.faces().count(), 257);
    let file_is_durable =
        super::super::super::super::font::file_stamp_durable_reusable(&first_path);
    let expected_attempts = if file_is_durable { 2 } else { 1 };
    let expected_calls = if file_is_durable { 4 } else { 2 };
    let first_face_id = first_face(&database)?;
    let calls = Arc::new(AtomicUsize::new(0));
    let mut options = counted_options(Arc::clone(&calls));
    let attempts = AtomicUsize::new(0);
    let result = super::super::super::super::with_validated_tree_parse(|| {
        attempts.fetch_add(1, Ordering::Relaxed);
        let first = select(&mut options, &database, 'x', &[]);
        let repeated = select(&mut options, &database, 'x', &[]);
        (first, repeated)
    });
    assert_eq!(attempts.load(Ordering::Relaxed), expected_attempts);
    assert_eq!(result.0, result.1);
    assert_eq!(result.0, Some(first_face_id));
    assert_eq!(calls.load(Ordering::Relaxed), expected_calls);
    Ok(())
}

fn duplicate_sfnt_as_two_face_ttc(sfnt: &[u8]) -> TestResult<Vec<u8>> {
    let table_count = u16::from_be_bytes([sfnt[4], sfnt[5]]) as usize;
    let second_offset = 20 + sfnt.len();
    let mut ttc = Vec::with_capacity(second_offset + sfnt.len());
    ttc.extend_from_slice(b"ttcf");
    ttc.extend_from_slice(&[0, 1, 0, 0]);
    ttc.extend_from_slice(&2_u32.to_be_bytes());
    ttc.extend_from_slice(&20_u32.to_be_bytes());
    ttc.extend_from_slice(&(second_offset as u32).to_be_bytes());
    append_sfnt_face(&mut ttc, sfnt, table_count, 20)?;
    append_sfnt_face(&mut ttc, sfnt, table_count, second_offset)?;
    Ok(ttc)
}

fn append_sfnt_face(
    ttc: &mut Vec<u8>,
    sfnt: &[u8],
    table_count: usize,
    file_offset: usize,
) -> TestResult<()> {
    let start = ttc.len();
    ttc.extend_from_slice(sfnt);
    for table in 0..table_count {
        let offset_field = start + 12 + table * 16 + 8;
        let old_offset = u32::from_be_bytes(
            ttc.get(offset_field..offset_field + 4)
                .ok_or("malformed SFNT table directory")?
                .try_into()?,
        );
        ttc[offset_field..offset_field + 4]
            .copy_from_slice(&(old_offset + file_offset as u32).to_be_bytes());
    }
    Ok(())
}

fn two_face_file_database() -> TestResult<(FontDirectory, std::path::PathBuf, Arc<Database>)> {
    let directory = FontDirectory::create()?;
    let path = directory.0.join("two-faces.ttc");
    std::fs::write(&path, duplicate_sfnt_as_two_face_ttc(FONT)?)?;
    let mut font_database = Database::new();
    font_database.load_font_source(Source::File(path.clone()));
    Ok((directory, path, Arc::new(font_database)))
}

fn assert_two_faces_from_path(database: &Database, path: &std::path::Path) -> ID {
    let faces: Vec<_> = database.faces().collect();
    assert_eq!(faces.len(), 2);
    assert!(
        faces
            .iter()
            .all(|face| { matches!(&face.source, Source::File(face_path) if face_path == path) })
    );
    faces[0].id
}

type TwoFaceSelection = (Option<ID>, Option<ID>);
type TwoFaceSelectionOutcome = (TwoFaceSelection, usize, usize);

fn run_two_face_mutating_selection(
    path: &std::path::Path,
    options: &mut usvg::Options<'_>,
    database: &Arc<Database>,
    calls: &Arc<AtomicUsize>,
) -> TestResult<TwoFaceSelectionOutcome> {
    let attempts = AtomicUsize::new(0);
    let mut mutation = Ok(());
    let result = super::super::super::super::with_validated_tree_parse(|| {
        attempts.fetch_add(1, Ordering::Relaxed);
        let first = select(options, database, 'x', &[]);
        let repeated = select(options, database, 'x', &[]);
        if attempts.load(Ordering::Relaxed) == 1 {
            mutation = std::fs::write(path, b"changed collection generation");
        }
        (first, repeated)
    });
    mutation?;
    Ok((
        result,
        calls.load(Ordering::Relaxed),
        attempts.load(Ordering::Relaxed),
    ))
}

#[test]
fn multiple_faces_keep_one_generation_with_durable_retry_or_weak_uncached_parse() -> TestResult<()>
{
    let (_directory, path, database) = two_face_file_database()?;
    let file_is_durable = super::file_is_durable(&path);
    let expected_attempts = if file_is_durable { 2 } else { 1 };
    let expected_calls = super::selector_calls_for_file(&path);
    let first_face_id = assert_two_faces_from_path(&database, &path);
    let generation_before =
        super::super::super::super::font::font_source_generation(&database, first_face_id);

    let calls = Arc::new(AtomicUsize::new(0));
    let mut options = counted_options(Arc::clone(&calls));
    let (result, selector_calls, attempts) =
        run_two_face_mutating_selection(&path, &mut options, &database, &calls)?;
    assert_two_face_selection(
        &result,
        selector_calls,
        attempts,
        expected_calls,
        expected_attempts,
        first_face_id,
    );
    let generation_after =
        super::super::super::super::font::font_source_generation(&database, first_face_id);
    assert!(generation_after.is_file());
    assert!(generation_after.reusable());
    assert_ne!(generation_before, generation_after);
    Ok(())
}

fn assert_two_face_selection(
    result: &TwoFaceSelection,
    selector_calls: usize,
    attempts: usize,
    expected_calls: usize,
    expected_attempts: usize,
    expected_face: ID,
) {
    assert_eq!(selector_calls, expected_calls);
    assert_eq!(result.0, result.1);
    assert_eq!(result.0, Some(expected_face));
    assert_eq!(attempts, expected_attempts);
}
