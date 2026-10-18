use super::super::FontShape;
use super::{SHAPED_RUN_CACHE, ShapedRunKey, cached_shaped_run, cached_shaped_run_width};
use crate::markdown::svg_rasterize::font::bundled_font_db;
use resvg::usvg::fontdb::{Database, ID, Source};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const FONT_SIZE: f32 = 16.0;
const FIRST_TEXT: &str = "AV";
const SECOND_TEXT: &str = "II";

struct Widths {
    first: f32,
    second: f32,
    stable: f32,
}

pub(super) fn assert_file_replacement_rejects_all_stale_runs() {
    with_file_database(|database, face, path| {
        with_stable_face(database, |stable| {
            assert_ne!(face, stable);
            with_width(database, face, FIRST_TEXT, |first| {
                with_width(database, face, SECOND_TEXT, |second| {
                    with_width(database, stable, FIRST_TEXT, |stable_width| {
                        let widths = Widths {
                            first,
                            second,
                            stable: stable_width,
                        };
                        replace_and_assert(database, face, stable, path, widths);
                    });
                });
            });
        });
    });
}

fn with_stable_face(database: &Arc<Database>, operation: impl FnOnce(ID)) {
    let face = database
        .faces()
        .find_map(|face| matches!(&face.source, Source::Binary(_)).then_some(face.id));
    assert!(face.is_some_and(|face| {
        operation(face);
        true
    }));
}

fn with_file_database(operation: impl FnOnce(&Arc<Database>, ID, &Path)) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "krr-shaped-generation-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(std::fs::create_dir(&directory).is_ok());
    let path = directory.join("font.ttf");
    assert!(std::fs::write(&path, FONT_BYTES).is_ok());
    let mut database = (*bundled_font_db()).clone();
    database.load_font_source(Source::File(path.clone()));
    let face = database.faces().find_map(|face| {
        matches!(&face.source, Source::File(source) if source == &path).then_some(face.id)
    });
    let database = Arc::new(database);
    assert!(face.is_some_and(|face| {
        operation(&database, face, &path);
        true
    }));
    assert!(std::fs::remove_file(path).is_ok());
    assert!(std::fs::remove_dir(directory).is_ok());
}

fn with_width(database: &Arc<Database>, face: ID, text: &str, operation: impl FnOnce(f32)) {
    assert!(
        cached_shaped_run(database, face, text, font(), None).is_some_and(|width| {
            assert!(width.is_finite() && width > 0.0);
            operation(width);
            true
        })
    );
}

fn font() -> FontShape {
    FontShape::new(FONT_SIZE, 400, false)
}

fn key(face: ID, text: &str) -> ShapedRunKey {
    ShapedRunKey::new(face, text, font(), None)
}

fn replace_and_assert(database: &Arc<Database>, face: ID, stable: ID, path: &Path, widths: Widths) {
    for (text, width) in [(FIRST_TEXT, widths.first), (SECOND_TEXT, widths.second)] {
        assert_eq!(
            cached_shaped_run_width(database, &key(face, text)),
            Some(width)
        );
    }
    let generation = super::font_source_generation(database, face);
    let replacement = path.with_extension("replacement.ttf");
    let bytes = [FONT_BYTES, &[0_u8][..]].concat();
    assert!(std::fs::write(&replacement, bytes).is_ok());
    assert!(std::fs::rename(replacement, path).is_ok());
    assert_ne!(generation, super::font_source_generation(database, face));
    assert_changed_face_only(database, face, stable, &widths);
    assert_current_generation_repopulates(database, face, &widths);
}

fn assert_changed_face_only(database: &Arc<Database>, face: ID, stable: ID, widths: &Widths) {
    /* WHY: 正常フォントの原子的な置換で、同 face の別文字列幅も無効になることを観測する。 */
    for text in [FIRST_TEXT, SECOND_TEXT] {
        assert_eq!(cached_shaped_run_width(database, &key(face, text)), None);
    }
    assert_eq!(
        cached_shaped_run_width(database, &key(stable, FIRST_TEXT)),
        Some(widths.stable)
    );
    assert_cache_counts(database, 1, 1, FIRST_TEXT.len());
}

fn assert_current_generation_repopulates(database: &Arc<Database>, face: ID, widths: &Widths) {
    for (text, width) in [(FIRST_TEXT, widths.first), (SECOND_TEXT, widths.second)] {
        assert_eq!(
            cached_shaped_run(database, face, text, font(), None),
            Some(width)
        );
    }
    assert_cache_counts(database, 3, 2, FIRST_TEXT.len() * 2 + SECOND_TEXT.len());
}

fn assert_cache_counts(database: &Arc<Database>, widths: usize, faces: usize, bytes: usize) {
    let bytes = bytes + widths * std::mem::size_of::<super::CachedShapedRun>();
    SHAPED_RUN_CACHE.with(|cache| {
        assert!(
            cache
                .borrow()
                .iter()
                .find(|entry| {
                    entry
                        .database
                        .upgrade()
                        .is_some_and(|cached| Arc::ptr_eq(&cached, database))
                })
                .is_some_and(|entry| {
                    assert_eq!(entry.values.len(), widths);
                    assert_eq!(entry.generations.len(), faces);
                    assert_eq!(entry.retained_bytes, bytes);
                    true
                })
        );
    });
}

#[test]
fn last_width_removal_releases_its_face_generation() {
    with_file_database(|database, face, _| {
        with_width(database, face, FIRST_TEXT, |_| {
            assert_cache_counts(database, 1, 1, FIRST_TEXT.len());
            SHAPED_RUN_CACHE.with(|cache| {
                let mut entries = cache.borrow_mut();
                let entry = super::shaped_run_cache_entry(&mut entries, database);
                super::generation::remove_width(entry, &key(face, FIRST_TEXT));
            });
            assert_cache_counts(database, 0, 0, 0);
            assert!(cached_shaped_run_width(database, &key(face, FIRST_TEXT)).is_none());
            assert!(cached_shaped_run(database, face, FIRST_TEXT, font(), None).is_some());
            assert_cache_counts(database, 1, 1, FIRST_TEXT.len());
        });
    });
}

#[test]
fn svg_endpoint_cache_rejects_atomic_font_file_replacement() {
    with_file_database(|database, face, path| {
        let first = super::cached_svg_run(database, face, FIRST_TEXT, font());
        assert!(first.is_some());
        let record = super::cached_shaped_run_record(database, &key(face, FIRST_TEXT));
        assert!(record.is_some_and(|run| run.svg_endpoints.is_some()));
        let replacement = path.with_extension("svg-replacement.ttf");
        assert!(std::fs::write(&replacement, [FONT_BYTES, &[0_u8][..]].concat()).is_ok());
        assert!(std::fs::rename(replacement, path).is_ok());
        assert!(super::cached_shaped_run_record(database, &key(face, FIRST_TEXT)).is_none());
        let current = super::cached_svg_run(database, face, FIRST_TEXT, font());
        assert!(current.is_some());
        let record = super::cached_shaped_run_record(database, &key(face, FIRST_TEXT));
        assert!(record.is_some_and(|run| run.svg_endpoints.is_some()));
    });
}

#[test]
fn replacement_between_validation_and_insertion_cannot_stamp_old_metrics_as_new()
-> Result<(), String> {
    for svg in [false, true] {
        let mut result = Ok(());
        with_file_database(|database, face, path| {
            result = assert_replacement_race(database, face, path, svg);
        });
        result?;
    }
    Ok(())
}

fn assert_replacement_race(
    database: &Arc<Database>,
    face: ID,
    path: &Path,
    svg: bool,
) -> Result<(), String> {
    let before = super::font_source_generation(database, face);
    let (advance, ascent, descent) = measured_file_metrics(database, face)?;
    let validated = super::font_source_generation(database, face);
    assert_eq!(before, validated);
    let current = replace_file_after_validation(database, face, path, &validated)?;
    let run_key = key(face, FIRST_TEXT);
    cache_validated_metrics(
        database,
        run_key.clone(),
        &validated,
        advance,
        ascent,
        descent,
        svg,
    );
    assert_current_generation_rejects_stale_run(database, face, &run_key, &validated, &current);
    Ok(())
}

fn measured_file_metrics(database: &Arc<Database>, face: ID) -> Result<(f32, f32, f32), String> {
    let metrics = super::super::harfrust::shape_svg_run_metrics_with_face(
        database,
        face,
        FIRST_TEXT,
        font(),
        false,
    )
    .ok_or("actual file must shape before replacement")?
    .0;
    Ok((metrics.advance, metrics.ascent, metrics.descent))
}

fn replace_file_after_validation(
    database: &Arc<Database>,
    face: ID,
    path: &Path,
    validated: &super::FontSourceGeneration,
) -> Result<super::FontSourceGeneration, String> {
    let replacement = path.with_extension("between-validation-and-insertion.ttf");
    std::fs::write(&replacement, [FONT_BYTES, &[0_u8][..]].concat())
        .map_err(|error| error.to_string())?;
    std::fs::rename(replacement, path).map_err(|error| error.to_string())?;
    let current = super::font_source_generation(database, face);
    assert_ne!(validated, &current);
    Ok(current)
}

fn cache_validated_metrics(
    database: &Arc<Database>,
    run_key: ShapedRunKey,
    validated: &super::FontSourceGeneration,
    advance: f32,
    ascent: f32,
    descent: f32,
    svg: bool,
) {
    if svg {
        super::cache_shaped_run_record(
            database,
            run_key,
            super::CachedShapedRun {
                advance,
                svg_endpoints: Some((ascent, descent)),
                ink_bounds: None,
            },
            validated.clone(),
        );
    } else {
        super::cache_shaped_run_width(database, run_key, advance, validated.clone());
    }
}

fn assert_current_generation_rejects_stale_run(
    database: &Arc<Database>,
    face: ID,
    run_key: &ShapedRunKey,
    validated: &super::FontSourceGeneration,
    current: &super::FontSourceGeneration,
) {
    assert!(super::cached_shaped_run_record_for_generation(database, run_key, validated).is_some());
    assert!(super::cached_shaped_run_record_for_generation(database, run_key, current).is_none());
    assert!(super::cached_svg_run(database, face, FIRST_TEXT, font()).is_some());
}
