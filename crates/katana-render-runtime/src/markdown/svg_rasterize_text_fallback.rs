use super::font::{append_font_run, matching_fallback_with_probe};
#[path = "svg_rasterize_text_fallback_generation.rs"]
mod fallback_generation;
use fallback_generation::{
    FallbackResolution, FileGenerationDependencies, probe_with_generation, selection_for_cache,
    selection_is_current,
};
#[path = "svg_rasterize_text_fallback_cache.rs"]
mod fallback_cache;
use fallback_cache::{HtmlFallbackKey, insert_cached, lookup_cached, remove_cached};
#[cfg(test)]
use fallback_generation::{FontSourceGeneration, MAX_FILE_DEPENDENCIES};
use resvg::usvg;
use std::collections::HashMap;
use std::sync::Arc;

pub(super) fn html_font_runs(
    database: &Arc<usvg::fontdb::Database>,
    base_face_id: usvg::fontdb::ID,
    text: &str,
    requested_weight: u16,
    requested_italic: bool,
) -> Vec<(usvg::fontdb::ID, String)> {
    let mut runs: Vec<(usvg::fontdb::ID, String)> = Vec::new();
    let mut render_faces = HashMap::new();
    for character in text.chars() {
        let face_id = cached_html_face(
            database,
            base_face_id,
            character,
            requested_weight,
            requested_italic,
            &mut render_faces,
        );
        append_font_run(&mut runs, face_id, character);
    }
    runs
}

fn cached_html_face(
    database: &Arc<usvg::fontdb::Database>,
    base_face_id: usvg::fontdb::ID,
    character: char,
    requested_weight: u16,
    requested_italic: bool,
    render_faces: &mut HashMap<HtmlFallbackKey, usvg::fontdb::ID>,
) -> usvg::fontdb::ID {
    let key = HtmlFallbackKey {
        base_face_id,
        character,
        requested_weight,
        requested_italic,
    };
    cached_html_face_for_key(database, key, render_faces)
}

fn cached_html_face_for_key(
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
    render_faces: &mut HashMap<HtmlFallbackKey, usvg::fontdb::ID>,
) -> usvg::fontdb::ID {
    if let Some(face_id) = render_faces.get(&key) {
        return *face_id;
    }
    let cached = lookup_cached(database, key);
    if let Some(selection) = cached {
        if selection_is_current(database, &selection) {
            render_faces.insert(key, selection.face_id);
            return selection.face_id;
        }
        remove_cached(database, key);
    }
    let resolution = resolve_html_fallback_face(database, key);
    let face_id = resolution.face_id;
    if let Some(selection) = selection_for_cache(resolution) {
        insert_cached(database, key, selection);
    }
    render_faces.insert(key, face_id);
    face_id
}

fn resolve_html_fallback_face(
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
) -> FallbackResolution {
    let mut dependencies = FileGenerationDependencies::new();
    let (face_id, cacheable) = {
        let mut probe = |id, ch| probe_with_generation(database, id, ch, &mut dependencies);
        let support = probe(key.base_face_id, key.character);
        if support == Some(true) {
            (key.base_face_id, true)
        } else {
            let (fallback, cacheable) = matching_fallback_with_probe(
                database,
                key.base_face_id,
                key.character,
                key.requested_weight,
                key.requested_italic,
                probe,
            );
            (
                fallback.unwrap_or(key.base_face_id),
                support.is_some() && cacheable,
            )
        }
    };
    FallbackResolution::new(face_id, cacheable, dependencies)
}

#[cfg(test)]
mod tests {
    use super::super::font::{font_has_char, font_source_generation, matching_font_face};
    use super::{
        HtmlFallbackKey, MAX_FILE_DEPENDENCIES, cached_html_face,
        fallback_cache::{
            HtmlFallbackCacheEntry, MAX_CACHED_HTML_FALLBACK_DATABASES,
            MAX_CACHED_HTML_FALLBACK_FACES, insert_existing_html_face, insert_html_face,
        },
        fallback_generation::HtmlFallbackSelection,
        html_font_runs,
    };
    use crate::markdown::svg_rasterize::font::{bundled_font_db, html_font_db_for_text};
    use resvg::usvg;
    use std::collections::{HashMap, HashSet};
    use std::sync::{
        Arc, Weak,
        atomic::{AtomicUsize, Ordering},
    };

    static TEMP_FONT_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempFontFile(std::path::PathBuf);

    impl Drop for TempFontFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn html_font_fallback_is_scoped_to_the_current_database() {
        let bundled = bundled_font_db();
        let bundled_base = matching_font_face(&bundled, "Noto Sans", 400, false);
        let bundled_runs =
            bundled_base.map(|base| html_font_runs(&bundled, base, "日", 400, false));
        let html = html_font_db_for_text("Noto Sans", "日");
        let html_base = matching_font_face(&html, "Noto Sans", 400, false);
        let used_html_fallback = html_base.is_some_and(|base| {
            !font_has_char(&html, base, '日')
                && html_font_runs(&html, base, "日本日本", 400, false)
                    .first()
                    .is_some_and(|(fallback, _)| *fallback != base)
        });

        assert!(bundled_runs.is_some_and(|runs| runs.len() == 1));
        assert!(used_html_fallback);
    }

    #[test]
    fn html_font_fallback_accepts_requested_weight_and_style() {
        let html = html_font_db_for_text("Noto Sans", "日");
        let requested_faces = [(700, false), (700, true)]
            .into_iter()
            .map(|(weight, italic)| {
                matching_font_face(&html, "Noto Sans", weight, italic).is_some_and(|base| {
                    html_font_runs(&html, base, "日", weight, italic)
                        .first()
                        .is_some_and(|(fallback, _)| {
                            *fallback != base && font_has_char(&html, *fallback, '日')
                        })
                })
            })
            .collect::<Vec<_>>();

        assert_eq!(requested_faces, [true, true]);
    }

    #[test]
    fn html_font_fallback_is_memoized_within_a_render() -> Result<(), String> {
        let database = html_font_db_for_text("Noto Sans", "日");
        let base_face_id = matching_font_face(&database, "Noto Sans", 400, false)
            .ok_or("HTML database must contain the requested base face")?;
        let mut render_faces = HashMap::new();

        let first = cached_html_face(&database, base_face_id, '日', 400, false, &mut render_faces);
        let mut next_render_faces = HashMap::new();
        let second = cached_html_face(
            &database,
            base_face_id,
            '日',
            400,
            false,
            &mut next_render_faces,
        );

        assert_eq!(first, second);
        assert_eq!(render_faces.len(), 1);
        assert_eq!(next_render_faces.len(), 1);
        Ok(())
    }

    #[test]
    fn same_style_file_fallback_change_rechecks_the_selected_face()
    -> Result<(), Box<dyn std::error::Error>> {
        let (face, original, replacement) = cmap_font_fixture()?;
        let (path, _guard) = temp_font_file("selected", &original)?;
        let (database, base, file_face, stable_face) =
            fallback_file_database(face, &original, &replacement, &path);
        assert_eq!(
            html_font_runs(&database, base, "A", 400, false)[0].0,
            file_face
        );
        std::fs::write(path, replacement)?;
        assert_eq!(
            html_font_runs(&database, base, "A", 400, false)[0].0,
            stable_face
        );
        Ok(())
    }

    #[test]
    fn same_style_file_base_change_rechecks_base_support() -> Result<(), Box<dyn std::error::Error>>
    {
        let (face, original, replacement) = cmap_font_fixture()?;
        let (path, _guard) = temp_font_file("base", &replacement)?;
        let (database, base, fallback) = base_file_database(face, &original, &path);
        assert_eq!(
            html_font_runs(&database, base, "A", 400, false)[0].0,
            fallback
        );
        std::fs::write(path, original)?;
        assert_eq!(html_font_runs(&database, base, "A", 400, false)[0].0, base);
        Ok(())
    }

    #[test]
    fn same_style_missing_selected_file_rechecks_the_fallback()
    -> Result<(), Box<dyn std::error::Error>> {
        let (face, original, replacement) = cmap_font_fixture()?;
        let (path, _guard) = temp_font_file("selected-missing", &original)?;
        let (database, base, file_face, stable_face) =
            fallback_file_database(face, &original, &replacement, &path);
        assert_eq!(
            html_font_runs(&database, base, "A", 400, false)[0].0,
            file_face
        );
        std::fs::remove_file(path)?;
        assert_eq!(
            html_font_runs(&database, base, "A", 400, false)[0].0,
            stable_face
        );
        Ok(())
    }

    #[test]
    fn same_style_consulted_better_file_change_rechecks_the_winner()
    -> Result<(), Box<dyn std::error::Error>> {
        let (face, original, unsupported) = cmap_font_fixture()?;
        let (path, _guard) = temp_font_file("better", &unsupported)?;
        let (database, base, better, lower) =
            better_file_database(face, &original, &unsupported, &path);
        assert_eq!(html_font_runs(&database, base, "A", 400, false)[0].0, lower);
        std::fs::write(path, original)?;
        assert_eq!(
            html_font_runs(&database, base, "A", 400, false)[0].0,
            better
        );
        Ok(())
    }

    #[test]
    fn unavailable_source_generations_cannot_reuse_fallback_choices()
    -> Result<(), Box<dyn std::error::Error>> {
        let empty = usvg::fontdb::Database::new();
        assert!(!font_source_generation(&empty, usvg::fontdb::ID::default()).reusable());
        let bundled = bundled_font_db();
        let mut face = bundled
            .faces()
            .next()
            .ok_or("bundled face missing")?
            .clone();
        let mut database = usvg::fontdb::Database::new();
        let missing = std::env::temp_dir().join(format!(
            "krr-fallback-missing-generation-{}.ttf",
            std::process::id()
        ));
        face.source = usvg::fontdb::Source::File(missing);
        let id = database.push_face_info(face);
        assert!(!font_source_generation(&database, id).reusable());
        Ok(())
    }

    #[test]
    fn consulted_file_generations_are_bounded_and_fit_the_cache_budget()
    -> Result<(), Box<dyn std::error::Error>> {
        let (path, _guard) = temp_font_file("dependency-bound", b"file stamp")?;
        let mut face = bundled_font_db()
            .faces()
            .next()
            .ok_or("bundled face missing")?
            .clone();
        face.source = usvg::fontdb::Source::File(path);
        let mut database = usvg::fontdb::Database::new();
        let ids = (0..=MAX_FILE_DEPENDENCIES)
            .map(|_| database.push_face_info(face.clone()))
            .collect::<Vec<_>>();
        let database = Arc::new(database);
        let mut dependencies = super::FileGenerationDependencies::new();
        for id in ids {
            let generation = font_source_generation(&database, id);
            dependencies.observe(id, generation.clone(), generation);
        }
        assert_eq!(dependencies.entries.len(), MAX_FILE_DEPENDENCIES);
        assert!(!dependencies.reusable);
        assert_file_generation_memory_bound();
        Ok(())
    }

    fn assert_file_generation_memory_bound() {
        let dependency_size =
            std::mem::size_of::<(usvg::fontdb::ID, super::FontSourceGeneration)>();
        let dependencies = MAX_CACHED_HTML_FALLBACK_DATABASES
            * MAX_CACHED_HTML_FALLBACK_FACES
            * MAX_FILE_DEPENDENCIES
            * dependency_size;
        let cache_slots = MAX_CACHED_HTML_FALLBACK_DATABASES * MAX_CACHED_HTML_FALLBACK_FACES * 2;
        let cache_entry_size = std::mem::size_of::<(HtmlFallbackKey, HtmlFallbackSelection)>();
        let fallback_map = cache_slots * (cache_entry_size + 1 + 16);
        let glyph_capacity = 65_536 * 4;
        let glyph_table = MAX_CACHED_HTML_FALLBACK_DATABASES
            * (glyph_capacity * (std::mem::size_of::<(usvg::fontdb::ID, char, bool)>() + 1)
                + 16
                + std::mem::size_of::<HashMap<(usvg::fontdb::ID, char), bool>>());
        let stamp_capacity = 1_024 * 2;
        let stamp_table = MAX_CACHED_HTML_FALLBACK_DATABASES
            * (stamp_capacity * (dependency_size + 1)
                + 16
                + std::mem::size_of::<HashMap<usvg::fontdb::ID, super::FontSourceGeneration>>());
        let total = glyph_table + stamp_table + dependencies + fallback_map;
        assert!(dependency_size <= 128);
        assert!(total < 64 * 1024 * 1024);
    }

    type CmapFontFixture = (usvg::fontdb::FaceInfo, Vec<u8>, Vec<u8>);
    type FixtureResult<T> = Result<T, Box<dyn std::error::Error>>;

    fn cmap_font_fixture() -> FixtureResult<CmapFontFixture> {
        let bundled = bundled_font_db();
        let face = bundled
            .faces()
            .next()
            .ok_or("bundled face missing")?
            .clone();
        let original = bundled
            .with_face_data(face.id, |data, _| data.to_vec())
            .ok_or("font bytes missing")?;
        let replacement = font_with_only_b_cmap(&original)?;
        Ok((face, original, replacement))
    }

    #[test]
    fn cmap_fixture_rejects_a_missing_table_tag() -> FixtureResult<()> {
        let (_, original, _) = cmap_font_fixture()?;
        assert!(table_record(&original, b"miss").is_err());
        Ok(())
    }

    fn temp_font_file(
        prefix: &str,
        bytes: &[u8],
    ) -> Result<(std::path::PathBuf, TempFontFile), std::io::Error> {
        let path = std::env::temp_dir().join(format!(
            "krr-fallback-generation-{prefix}-{}-{}.ttf",
            std::process::id(),
            TEMP_FONT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes)?;
        Ok((path.clone(), TempFontFile(path)))
    }

    fn fallback_file_database(
        face: usvg::fontdb::FaceInfo,
        bytes: &[u8],
        unsupported_bytes: &[u8],
        path: &std::path::Path,
    ) -> (
        Arc<usvg::fontdb::Database>,
        usvg::fontdb::ID,
        usvg::fontdb::ID,
        usvg::fontdb::ID,
    ) {
        let mut base_face = face.clone();
        base_face.source = usvg::fontdb::Source::Binary(Arc::new(unsupported_bytes.to_vec()));
        let mut selected_face = face.clone();
        selected_face.source = usvg::fontdb::Source::File(path.to_path_buf());
        let mut stable_face = face;
        stable_face.source = usvg::fontdb::Source::Binary(Arc::new(bytes.to_vec()));
        let mut database = usvg::fontdb::Database::new();
        let base = database.push_face_info(base_face);
        let selected = database.push_face_info(selected_face);
        let stable = database.push_face_info(stable_face);
        (Arc::new(database), base, selected, stable)
    }

    fn base_file_database(
        face: usvg::fontdb::FaceInfo,
        bytes: &[u8],
        path: &std::path::Path,
    ) -> (
        Arc<usvg::fontdb::Database>,
        usvg::fontdb::ID,
        usvg::fontdb::ID,
    ) {
        let mut base_face = face.clone();
        base_face.source = usvg::fontdb::Source::File(path.to_path_buf());
        let mut fallback_face = face;
        fallback_face.source = usvg::fontdb::Source::Binary(Arc::new(bytes.to_vec()));
        let mut database = usvg::fontdb::Database::new();
        let base = database.push_face_info(base_face);
        let fallback = database.push_face_info(fallback_face);
        (Arc::new(database), base, fallback)
    }

    fn better_file_database(
        face: usvg::fontdb::FaceInfo,
        original: &[u8],
        unsupported: &[u8],
        path: &std::path::Path,
    ) -> (
        Arc<usvg::fontdb::Database>,
        usvg::fontdb::ID,
        usvg::fontdb::ID,
        usvg::fontdb::ID,
    ) {
        let mut base_face = face.clone();
        base_face.source = usvg::fontdb::Source::Binary(Arc::new(unsupported.to_vec()));
        let mut better_face = face.clone();
        better_face.source = usvg::fontdb::Source::File(path.to_path_buf());
        let mut lower_face = face;
        lower_face.source = usvg::fontdb::Source::Binary(Arc::new(original.to_vec()));
        lower_face.weight = usvg::fontdb::Weight(700);
        let mut database = usvg::fontdb::Database::new();
        let base = database.push_face_info(base_face);
        let better = database.push_face_info(better_face);
        let lower = database.push_face_info(lower_face);
        (Arc::new(database), base, better, lower)
    }

    fn font_with_only_b_cmap(font: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let (record, offset) = table_record(font, b"cmap")?;
        let (_, head) = table_record(font, b"head")?;
        let glyph = rustybuzz::ttf_parser::Face::parse(font, 0)
            .map_err(|_| "font invalid")?
            .glyph_index('B')
            .ok_or("B glyph missing")?
            .0;
        let cmap = b_cmap(glyph);
        let mut changed = font.to_vec();
        changed[offset..offset + cmap.len()].copy_from_slice(&cmap);
        changed[record + 4..record + 8].copy_from_slice(&sfnt_checksum(&cmap).to_be_bytes());
        changed[record + 12..record + 16].copy_from_slice(&(cmap.len() as u32).to_be_bytes());
        changed[head + 8..head + 12].fill(0);
        let adjustment = 0xB1B0_AFBA_u32.wrapping_sub(sfnt_checksum(&changed));
        changed[head + 8..head + 12].copy_from_slice(&adjustment.to_be_bytes());
        Ok(changed)
    }

    fn table_record(
        font: &[u8],
        tag: &[u8; 4],
    ) -> Result<(usize, usize), Box<dyn std::error::Error>> {
        let table_count = u16::from_be_bytes(font[4..6].try_into()?) as usize;
        for index in 0..table_count {
            let record = 12 + index * 16;
            let offset = u32::from_be_bytes(font[record + 8..record + 12].try_into()?) as usize;
            if &font[record..record + 4] == tag {
                return Ok((record, offset));
            }
        }
        Err("font table missing".into())
    }

    fn b_cmap(glyph: u16) -> Vec<u8> {
        let mut cmap = Vec::with_capacity(40);
        cmap.extend_from_slice(&[0, 0, 0, 1, 0, 3, 0, 10, 0, 0, 0, 12]);
        cmap.extend_from_slice(&12_u16.to_be_bytes());
        cmap.extend_from_slice(&0_u16.to_be_bytes());
        cmap.extend_from_slice(&28_u32.to_be_bytes());
        cmap.extend_from_slice(&0_u32.to_be_bytes());
        cmap.extend_from_slice(&1_u32.to_be_bytes());
        cmap.extend_from_slice(&u32::from('B').to_be_bytes());
        cmap.extend_from_slice(&u32::from('B').to_be_bytes());
        cmap.extend_from_slice(&u32::from(glyph).to_be_bytes());
        cmap
    }

    fn sfnt_checksum(bytes: &[u8]) -> u32 {
        bytes.chunks(4).fold(0_u32, |sum, chunk| {
            let mut word = [0; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            sum.wrapping_add(u32::from_be_bytes(word))
        })
    }

    type RecoveryFixture = (
        Arc<usvg::fontdb::Database>,
        usvg::fontdb::ID,
        usvg::fontdb::ID,
        Vec<u8>,
        std::path::PathBuf,
    );

    fn recovery_fixture(missing_base: bool) -> Result<RecoveryFixture, Box<dyn std::error::Error>> {
        let bundled = bundled_font_db();
        let original = bundled.faces().next().ok_or("bundled face missing")?;
        let bytes = bundled
            .with_face_data(original.id, |data, _| data.to_vec())
            .ok_or("bundled data missing")?;
        let path = std::env::temp_dir().join(format!(
            "krr-glyph-recovery-{}-{missing_base}.ttf",
            std::process::id()
        ));
        let mut database = usvg::fontdb::Database::new();
        let mut missing = original.clone();
        missing.source = usvg::fontdb::Source::File(path.clone());
        if !missing_base {
            let mut corrupt = original.clone();
            corrupt.source = usvg::fontdb::Source::Binary(Arc::new(vec![0; 32]));
            database.push_face_info(corrupt);
        }
        let recovered = database.push_face_info(missing);
        let fallback = database.push_face_info(original.clone());
        Ok((Arc::new(database), recovered, fallback, bytes, path))
    }

    #[test]
    fn missing_font_file_does_not_cache_the_fallback_choice()
    -> Result<(), Box<dyn std::error::Error>> {
        let (database, base, fallback, bytes, path) = recovery_fixture(true)?;
        let mut render_faces = HashMap::new();
        assert_eq!(
            cached_html_face(&database, base, 'A', 400, false, &mut render_faces),
            fallback
        );
        assert_eq!(render_faces.len(), 1);
        assert_eq!(
            cached_html_face(&database, base, 'A', 400, false, &mut render_faces),
            fallback
        );
        std::fs::write(&path, bytes)?;
        assert_eq!(
            cached_html_face(&database, base, 'A', 400, false, &mut render_faces),
            fallback
        );
        let mut next_render_faces = HashMap::new();
        let recovered = cached_html_face(&database, base, 'A', 400, false, &mut next_render_faces);
        std::fs::remove_file(path)?;
        assert_eq!(recovered, base);
        assert_eq!(next_render_faces.len(), 1);
        Ok(())
    }

    #[test]
    fn unavailable_fallback_candidate_is_retried_after_its_file_returns()
    -> Result<(), Box<dyn std::error::Error>> {
        let (database, candidate, fallback, bytes, path) = recovery_fixture(false)?;
        let base = database.faces().next().ok_or("base missing")?.id;
        let mut render_faces = HashMap::new();
        assert_eq!(
            cached_html_face(&database, base, 'A', 400, false, &mut render_faces),
            fallback
        );
        assert_eq!(render_faces.len(), 1);
        std::fs::write(&path, bytes)?;
        assert_eq!(
            cached_html_face(&database, base, 'A', 400, false, &mut render_faces),
            fallback
        );
        let mut next_render_faces = HashMap::new();
        let recovered = cached_html_face(&database, base, 'A', 400, false, &mut next_render_faces);
        std::fs::remove_file(path)?;
        assert_eq!(recovered, candidate);
        assert_eq!(next_render_faces.len(), 1);
        Ok(())
    }

    #[test]
    fn expired_html_fallback_database_entries_are_swept() {
        let expired_database = Arc::new(usvg::fontdb::Database::new());
        let expired_entry = HtmlFallbackCacheEntry {
            database: Arc::downgrade(&expired_database),
            faces: HashMap::new(),
        };
        drop(expired_database);

        let database = Arc::new(usvg::fontdb::Database::new());
        let key = fallback_test_key();
        let mut entries = vec![expired_entry];

        insert_html_face(
            &mut entries,
            &database,
            key,
            test_selection(key.base_face_id),
        );

        assert_eq!(entries.len(), 1);
        assert!(
            entries[0]
                .database
                .upgrade()
                .is_some_and(|cached| { Arc::ptr_eq(&cached, &database) })
        );
    }

    #[test]
    fn html_fallback_database_cache_evicts_oldest_entry_at_capacity() {
        let databases = (0..=MAX_CACHED_HTML_FALLBACK_DATABASES)
            .map(|_| Arc::new(usvg::fontdb::Database::new()))
            .collect::<Vec<_>>();
        let key = fallback_test_key();
        let mut entries = Vec::new();

        for database in &databases {
            insert_html_face(
                &mut entries,
                database,
                key,
                test_selection(key.base_face_id),
            );
        }

        assert_eq!(entries.len(), MAX_CACHED_HTML_FALLBACK_DATABASES);
        assert!(
            entries[0]
                .database
                .upgrade()
                .is_some_and(|cached| { Arc::ptr_eq(&cached, &databases[1]) })
        );
    }

    fn fallback_test_key() -> HtmlFallbackKey {
        HtmlFallbackKey {
            base_face_id: usvg::fontdb::ID::default(),
            character: 'x',
            requested_weight: 400,
            requested_italic: false,
        }
    }

    fn full_face_cache() -> HtmlFallbackCacheEntry {
        HtmlFallbackCacheEntry {
            database: Weak::new(),
            faces: (0..MAX_CACHED_HTML_FALLBACK_FACES)
                .map(|weight| {
                    (
                        HtmlFallbackKey {
                            base_face_id: usvg::fontdb::ID::default(),
                            character: 'x',
                            requested_weight: weight as u16,
                            requested_italic: false,
                        },
                        test_selection(usvg::fontdb::ID::default()),
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn html_fallback_face_cache_evicts_an_entry_at_capacity() {
        let mut entry = full_face_cache();
        let original_keys = entry.faces.keys().copied().collect::<HashSet<_>>();
        let new_key = HtmlFallbackKey {
            base_face_id: usvg::fontdb::ID::default(),
            character: 'x',
            requested_weight: MAX_CACHED_HTML_FALLBACK_FACES as u16,
            requested_italic: false,
        };

        insert_existing_html_face(&mut entry, new_key, test_selection(new_key.base_face_id));

        assert_eq!(entry.faces.len(), MAX_CACHED_HTML_FALLBACK_FACES);
        assert!(entry.faces.contains_key(&new_key));
        assert!(entry.faces.keys().any(|key| !original_keys.contains(key)));
    }

    fn test_selection(face_id: usvg::fontdb::ID) -> HtmlFallbackSelection {
        HtmlFallbackSelection {
            face_id,
            dependencies: Arc::from([]),
        }
    }
}
