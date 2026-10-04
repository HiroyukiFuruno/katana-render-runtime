use resvg::usvg::fontdb::{Database, ID};
use skrifa::{FontRef, charmap::MappingIndex};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const CORRUPTED_CMAP_SFNT_BYTES: [u8; 2] = [0, 1];
const APPENDED_MUTATION_BYTE: u8 = 0;
static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);

struct TempFont(PathBuf);

impl TempFont {
    fn write(bytes: &[u8]) -> Result<Self, String> {
        let serial = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "krr-usvg-cmap-probe-{}-{serial}.ttf",
            std::process::id()
        ));
        fs::write(&path, bytes).map_err(|error| error.to_string())?;
        Ok(Self(path))
    }
}

impl Drop for TempFont {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

struct FileDatabase {
    _font: TempFont,
    database: Arc<Database>,
    face_id: ID,
    face_index: u32,
}

fn file_database() -> Result<FileDatabase, String> {
    let font = TempFont::write(FONT_BYTES)?;
    let mut database = Database::new();
    database
        .load_font_file(&font.0)
        .map_err(|error| error.to_string())?;
    let face = database.faces().next().ok_or("font face missing")?;
    let face_id = face.id;
    let face_index = face.index;
    Ok(FileDatabase {
        _font: font,
        database: Arc::new(database),
        face_id,
        face_index,
    })
}

fn font_mapping(face_index: u32) -> Result<MappingIndex, String> {
    let font = FontRef::from_index(FONT_BYTES, face_index)
        .map_err(|error| format!("font parse: {error:?}"))?;
    Ok(MappingIndex::new(&font))
}

fn with_validated_scope<T>(operation: impl FnMut() -> T) -> T {
    super::super::super::with_validated_tree_parse(operation)
}

fn cache_test_guard() -> Result<
    (
        std::sync::MutexGuard<'static, ()>,
        super::super::storage::tests::CacheResetOnDrop,
    ),
    String,
> {
    let lock = super::super::storage::tests::begin()?;
    Ok((lock, super::super::storage::tests::CacheResetOnDrop))
}

#[test]
fn corrupt_cached_sfnt_is_removed_after_real_file_probe() -> Result<(), String> {
    let (_lock, _reset) = cache_test_guard()?;
    let file = file_database()?;
    let mut saved_generation = None;
    let result = with_validated_scope(|| {
        let generation =
            super::super::super::font::font_source_generation(&file.database, file.face_id);
        saved_generation = Some(generation.clone());
        if !super::super::storage::insert(
            &file.database,
            file.face_id,
            file.face_index,
            generation,
            CORRUPTED_CMAP_SFNT_BYTES.to_vec(),
            font_mapping(file.face_index).map_err(|_| ())?,
        ) {
            return Err(());
        }
        super::has_char(&file.database, file.face_id, 'A')
    });
    let generation = saved_generation.ok_or("file generation was not observed")?;
    assert_eq!(result, Err(()));
    assert!(
        super::super::storage::lookup(&file.database, file.face_id, file.face_index, &generation,)
            .map_err(|_| "cmap cache lock poisoned")?
            .is_none()
    );
    Ok(())
}

#[test]
fn poisoned_cache_preserves_real_file_probe_policy() -> Result<(), String> {
    let (_lock, _reset) = cache_test_guard()?;
    let file = file_database()?;
    let generation =
        super::super::super::font::font_source_generation(&file.database, file.face_id);
    let result = with_validated_scope(|| {
        super::super::storage::tests::poison_global_cache().map_err(|_| ())?;
        super::load_file_mapping(
            &file.database,
            file.face_id,
            file.face_index,
            &generation,
            'A',
        )
    });
    if generation.durable_reusable() {
        assert_eq!(result, Err(()));
    } else {
        assert_eq!(result, Ok(true));
    }
    Ok(())
}

#[test]
fn file_probe_is_rejected_outside_validated_batch() -> Result<(), String> {
    let (_lock, _reset) = cache_test_guard()?;
    let file = file_database()?;
    assert_eq!(super::has_char(&file.database, file.face_id, 'A'), Err(()));
    Ok(())
}

#[test]
fn changed_file_fails_private_generation_validation() -> Result<(), String> {
    let file = file_database()?;
    let generation =
        super::super::super::font::font_source_generation(&file.database, file.face_id);
    let mut append = OpenOptions::new()
        .append(true)
        .open(&file._font.0)
        .map_err(|error| error.to_string())?;
    append
        .write_all(&[APPENDED_MUTATION_BYTE])
        .map_err(|error| error.to_string())?;
    assert!(super::validate_file_generation(&file.database, file.face_id, &generation).is_err());
    Ok(())
}

#[test]
fn missing_selector_base_face_returns_none() {
    let selector = super::super::selector::html_selector();
    let mut database = Arc::new(Database::new());
    assert!(selector('A', &[ID::dummy()], &mut database).is_none());
}

#[test]
fn uncached_generation_distinguishes_binary_and_missing_face() -> Result<(), String> {
    let mut binary_database = Database::new();
    binary_database.load_font_data(FONT_BYTES.to_vec());
    let binary_face = binary_database
        .faces()
        .next()
        .ok_or("binary face missing")?;
    let binary = super::super::super::font::font_source_generation_uncached(
        &binary_database,
        binary_face.id,
    );
    assert!(!binary.is_file());
    assert!(binary.reusable());

    let missing_database = Database::new();
    let missing =
        super::super::super::font::font_source_generation_uncached(&missing_database, ID::dummy());
    assert!(!missing.is_file());
    assert!(!missing.reusable());
    Ok(())
}

#[test]
fn binary_font_probe_matches_bundled_font_character_map() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let database = Arc::new(database);
    let face_id = database
        .faces()
        .next()
        .ok_or("binary font face missing")?
        .id;

    assert_eq!(super::has_char(&database, face_id, 'A'), Ok(true));
    assert_eq!(super::has_char(&database, face_id, '\u{10ffff}'), Ok(false));
    Ok(())
}

fn database_with_two_file_fonts(
    first_font: &TempFont,
    second_font: &TempFont,
) -> Result<(Arc<Database>, ID, ID), String> {
    let mut font_database = Database::new();
    for font in [first_font, second_font] {
        font_database
            .load_font_file(&font.0)
            .map_err(|error| error.to_string())?;
    }
    let database = Arc::new(font_database);
    let face_ids: Vec<_> = database.faces().map(|face| face.id).collect();
    let base_face = *face_ids.first().ok_or("base font face missing")?;
    let fallback_face = *face_ids.get(1).ok_or("fallback font face missing")?;
    Ok((database, base_face, fallback_face))
}

#[test]
fn selector_falls_back_to_stock_when_cached_probe_is_unavailable() -> Result<(), String> {
    let first_font = TempFont::write(FONT_BYTES)?;
    let second_font = TempFont::write(FONT_BYTES)?;
    let (database, base_face, fallback_face) =
        database_with_two_file_fonts(&first_font, &second_font)?;
    assert_eq!(super::has_char(&database, fallback_face, 'A'), Err(()));
    let durable = super::super::super::font::file_stamp_durable_reusable(&second_font.0);
    assert_eq!(
        with_validated_scope(|| super::has_char(&database, fallback_face, 'A')),
        durable.then_some(true).ok_or(())
    );

    let excluded = [base_face];
    let stock = resvg::usvg::FontResolver::default_fallback_selector();
    let cached = super::super::selector::html_selector();
    let (stock_result, cached_result) = with_validated_scope(|| {
        (
            stock('A', &excluded, &mut Arc::clone(&database)),
            cached('A', &excluded, &mut Arc::clone(&database)),
        )
    });
    assert_eq!(stock_result, Some(fallback_face));
    assert_eq!(cached_result, stock_result);
    Ok(())
}

#[test]
fn stale_excluded_base_face_does_not_fall_back_to_bundled_face() -> Result<(), String> {
    let mut font_database = Database::new();
    font_database.load_font_data(FONT_BYTES.to_vec());
    let database = Arc::new(font_database);
    let face_id = database
        .faces()
        .next()
        .ok_or("bundled font face missing")?
        .id;
    let stale_base = [ID::dummy()];
    let cached = super::super::selector::html_selector();

    assert!(database.face(face_id).is_some());
    assert!(database.face(stale_base[0]).is_none());
    assert_eq!(cached('A', &stale_base, &mut Arc::clone(&database)), None);
    Ok(())
}

#[test]
fn missing_character_in_remaining_bundled_face_returns_none() -> Result<(), String> {
    let (_lock, _reset) = cache_test_guard()?;
    let first_font = TempFont::write(FONT_BYTES)?;
    let second_font = TempFont::write(FONT_BYTES)?;
    let mut font_database = Database::new();
    font_database
        .load_font_file(&first_font.0)
        .map_err(|error| error.to_string())?;
    font_database
        .load_font_file(&second_font.0)
        .map_err(|error| error.to_string())?;
    let database = Arc::new(font_database);
    let face_ids: Vec<_> = database.faces().map(|face| face.id).collect();
    assert_eq!(face_ids.len(), 2);
    let base_face = *face_ids.first().ok_or("base font face missing")?;
    let cached = super::super::selector::html_selector();
    let stock = resvg::usvg::FontResolver::default_fallback_selector();
    let missing = '\u{10ffff}';

    let cached_result =
        with_validated_scope(|| cached(missing, &[base_face], &mut Arc::clone(&database)));
    let stock_result = stock(missing, &[base_face], &mut Arc::clone(&database));
    assert_eq!(cached_result, None);
    assert_eq!(cached_result, stock_result);
    Ok(())
}
