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
fn two_hundred_fifty_seven_file_paths_retry_the_entire_parse_without_memo() -> TestResult<()> {
    let directory = FontDirectory::create()?;
    let first_path = directory.font_path(0);
    std::fs::write(&first_path, FONT)?;
    let mut database = Database::new();
    database.load_font_source(Source::File(first_path.clone()));
    for index in 1..257 {
        let path = directory.font_path(index);
        std::fs::hard_link(&first_path, &path)?;
        database.load_font_source(Source::File(path));
    }
    let database = Arc::new(database);
    assert_eq!(database.faces().count(), 257);
    let calls = Arc::new(AtomicUsize::new(0));
    let mut options = counted_options(Arc::clone(&calls));
    let attempts = AtomicUsize::new(0);
    let result = super::super::super::super::with_validated_tree_parse(|| {
        attempts.fetch_add(1, Ordering::Relaxed);
        let first = select(&mut options, &database, 'x', &[]);
        let repeated = select(&mut options, &database, 'x', &[]);
        (first, repeated)
    });
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
    assert_eq!(result.0, result.1);
    assert_eq!(calls.load(Ordering::Relaxed), 4);
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

#[test]
fn multiple_faces_from_one_file_share_one_generation_and_retry_as_a_unit() -> TestResult<()> {
    let (_directory, path, database) = two_face_file_database()?;
    let faces: Vec<_> = database.faces().collect();
    assert_eq!(faces.len(), 2);
    assert!(
        faces
            .iter()
            .all(|face| { matches!(&face.source, Source::File(face_path) if face_path == &path) })
    );

    let calls = Arc::new(AtomicUsize::new(0));
    let mut options = counted_options(Arc::clone(&calls));
    let attempts = AtomicUsize::new(0);
    let mut mutation = Ok(());
    let result = super::super::super::super::with_validated_tree_parse(|| {
        attempts.fetch_add(1, Ordering::Relaxed);
        let first = select(&mut options, &database, 'x', &[]);
        let repeated = select(&mut options, &database, 'x', &[]);
        if attempts.load(Ordering::Relaxed) == 1 {
            mutation = std::fs::write(&path, b"changed collection generation");
        }
        (first, repeated)
    });
    mutation?;
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
    assert_eq!(result.0, result.1);
    assert_eq!(calls.load(Ordering::Relaxed), 3);
    Ok(())
}
