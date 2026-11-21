use super::{FontSourceGeneration, font_source_generation, font_source_generation_uncached};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use resvg::usvg::fontdb::{Database, ID, Source};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const TEST_FAMILY: &str = "Noto Sans";
const TEST_SIZE: f32 = 16.0;
const TEST_WEIGHT: u16 = 400;
const TEST_ITALIC: bool = false;
const TEST_LETTER_SPACING: f32 = 0.0;
const REPETITIONS: usize = 5;

#[test]
fn requested_preflight_rejects_removed_real_font_face() -> Result<(), Box<dyn std::error::Error>> {
    let file = tempfile::NamedTempFile::new()?;
    std::fs::write(file.path(), FONT_BYTES)?;
    let (mut database, face) = file_database(file.path())?;
    assert_eq!(super::probe_font_has_char(&database, face, 'A'), Some(true));
    assert_eq!(noto_preflight(&database, face), Ok(false));
    Arc::make_mut(&mut database).remove_face(face);
    assert!(database.face(face).is_none());
    assert_eq!(noto_preflight(&database, face), Err((None, false)));
    Ok(())
}

fn noto_preflight(database: &Database, face: ID) -> Result<bool, (Option<ID>, bool)> {
    let attributes = super::fallback_attributes(TEST_WEIGHT, TEST_ITALIC);
    let mut probe = |id, character| super::probe_font_has_char(database, id, character);
    super::requested_preflight(database, face, 'A', attributes, &mut probe)
}

fn temp_font_path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |value| value.as_nanos());
    std::env::temp_dir().join(format!("krr-db-token-{}-{stamp}.ttf", std::process::id()))
}

fn file_database(path: &Path) -> Result<(Arc<Database>, ID), String> {
    let mut database = Database::new();
    database.load_font_source(Source::File(path.to_path_buf()));
    let database = Arc::new(database);
    let face = database.faces().next().ok_or("file face missing")?.id;
    Ok((database, face))
}

fn file_database_pair(first: &Path, second: &Path) -> Result<(Arc<Database>, ID, ID), String> {
    let mut database = Database::new();
    database.load_font_source(Source::File(first.to_path_buf()));
    database.load_font_source(Source::File(second.to_path_buf()));
    let ids = database.faces().map(|face| face.id).collect::<Vec<_>>();
    let [first_id, second_id] = ids.as_slice() else {
        return Err("two file faces missing".to_string());
    };
    Ok((Arc::new(database), *first_id, *second_id))
}

fn snapshot(database: &Arc<Database>) -> Vec<(ID, FontSourceGeneration)> {
    database
        .faces()
        .map(|face| (face.id, font_source_generation(database, face.id)))
        .collect()
}

fn snapshot_matches(database: &Arc<Database>, expected: &[(ID, FontSourceGeneration)]) -> bool {
    expected
        .iter()
        .all(|(id, generation)| font_source_generation_uncached(database, *id) == *generation)
}

fn assert_reusable(snapshot: &[(ID, FontSourceGeneration)]) {
    assert!(snapshot.iter().all(|(_, generation)| generation.reusable()));
}

#[test]
fn exact_database_snapshot_tracks_arc_mutation_and_new_identity() -> Result<(), String> {
    let path = temp_font_path();
    let unselected_path = temp_font_path();
    std::fs::write(&path, FONT_BYTES).map_err(|error| error.to_string())?;
    std::fs::write(&unselected_path, FONT_BYTES).map_err(|error| error.to_string())?;
    let (database, face, unselected_face) = file_database_pair(&path, &unselected_path)?;
    let expected =
        initial_snapshot_checks(&database, face, unselected_face, &path, &unselected_path)?;
    assert!(snapshot_matches(&database, &expected));
    let same_arc = Arc::clone(&database);
    assert!(Arc::ptr_eq(&database, &same_arc));
    let unselected_expected = expected
        .iter()
        .find(|(id, _)| *id == unselected_face)
        .ok_or("unselected face snapshot missing")?;
    assert_unselected_mutation(
        &database,
        unselected_face,
        &unselected_expected.1,
        &expected,
        &unselected_path,
    )?;
    assert_selected_deletion(&database, face, &expected, &path)?;
    assert_new_database_identity(&database, face)?;
    std::fs::remove_file(unselected_path).map_err(|error| error.to_string())?;
    Ok(())
}

fn initial_snapshot_checks(
    database: &Arc<Database>,
    face: ID,
    unselected_face: ID,
    path: &Path,
    unselected_path: &Path,
) -> Result<Vec<(ID, FontSourceGeneration)>, String> {
    let expected = snapshot(database);
    assert_reusable(&expected);
    let selected = expected
        .iter()
        .find(|(id, _)| *id == face)
        .ok_or("selected face snapshot missing")?;
    let unselected = expected
        .iter()
        .find(|(id, _)| *id == unselected_face)
        .ok_or("unselected face snapshot missing")?;
    assert_eq!(
        selected.1.durable_reusable(),
        super::file_stamp_durable_reusable(path)
    );
    assert_eq!(
        unselected.1.durable_reusable(),
        super::file_stamp_durable_reusable(unselected_path)
    );
    Ok(expected)
}

fn assert_unselected_mutation(
    database: &Arc<Database>,
    face: ID,
    expected_generation: &FontSourceGeneration,
    expected: &[(ID, FontSourceGeneration)],
    path: &Path,
) -> Result<(), String> {
    std::fs::write(path, &FONT_BYTES[..FONT_BYTES.len() - 1]).map_err(|error| error.to_string())?;
    assert_ne!(
        &font_source_generation_uncached(database, face),
        expected_generation
    );
    assert!(!snapshot_matches(database, expected));
    Ok(())
}

fn assert_selected_deletion(
    database: &Arc<Database>,
    face: ID,
    expected: &[(ID, FontSourceGeneration)],
    path: &Path,
) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|error| error.to_string())?;
    let deleted = font_source_generation_uncached(database, face);
    assert!(!deleted.reusable());
    assert!(!snapshot_matches(database, expected));
    Ok(())
}

fn assert_new_database_identity(database: &Arc<Database>, face: ID) -> Result<(), String> {
    let replacement = temp_font_path();
    std::fs::write(&replacement, FONT_BYTES).map_err(|error| error.to_string())?;
    let result = (|| {
        let (new_database, new_face) = file_database(&replacement)?;
        assert!(!Arc::ptr_eq(database, &new_database));
        assert_ne!(
            font_source_generation_uncached(database, face),
            font_source_generation_uncached(&new_database, new_face)
        );
        Ok(())
    })();
    std::fs::remove_file(replacement).map_err(|error| error.to_string())?;
    result
}

#[test]
fn exact_database_snapshot_and_real_width_cost_receipt() -> Result<(), String> {
    let requests = [
        ("latin", TEST_FAMILY, "A measured text"),
        ("cjk", TEST_FAMILY, "日本語の測定"),
        ("emoji", TEST_FAMILY, "A😀"),
        (
            "anonymous-fixture-family",
            "Helvetica Neue, Arial, 'Hiragino Kaku Gothic ProN', Meiryo, sans-serif",
            "A日本😀",
        ),
    ];
    let mut reports = Vec::with_capacity(requests.len());
    for (label, family, request_text) in requests {
        reports.push(measure_request(label, family, request_text)?);
    }
    let receipt = json!({
        "schema": "krr-test-exact-db-token",
        "tempfont_fixture": "NotoSans-Regular.ttf",
        "repetitions": REPETITIONS,
        "requests": reports,
    });
    println!("{receipt}");
    Ok(())
}

fn measure_request(
    label: &str,
    family: &str,
    request_text: &str,
) -> Result<serde_json::Value, String> {
    let database = super::super::html_font_db_for_text(family, request_text);
    let initial_snapshot = snapshot(&database);
    let (reusable_face_count, durable_face_count) = generation_counts(&initial_snapshot);
    let snapshot_times = measure_snapshot(&database);
    let width_times = measure_width(family, request_text)?;
    Ok(json!({
        "request": label,
        "font_family": family,
        "request_text": request_text,
        "face_count": database.faces().count(),
        "file_face_count": database.faces().filter(|face| matches!(face.source, Source::File(_))).count(),
        "reusable_face_count": reusable_face_count,
        "durable_face_count": durable_face_count,
        "snapshot_ns": durations(&snapshot_times),
        "width_ns": durations(&width_times),
    }))
}

fn generation_counts(snapshot: &[(ID, FontSourceGeneration)]) -> (usize, usize) {
    let reusable = snapshot
        .iter()
        .filter(|(_, generation)| generation.reusable())
        .count();
    let durable = snapshot
        .iter()
        .filter(|(_, generation)| generation.durable_reusable())
        .count();
    (reusable, durable)
}

fn measure_snapshot(database: &Arc<Database>) -> Vec<Duration> {
    (0..REPETITIONS)
        .map(|_| {
            let started = Instant::now();
            let values = snapshot(database);
            assert!(snapshot_matches(database, &values));
            started.elapsed()
        })
        .collect()
}

fn measure_width(family: &str, request_text: &str) -> Result<Vec<Duration>, String> {
    (0..REPETITIONS)
        .map(|_| {
            let started = Instant::now();
            SvgRasterizeOps::measure_html_text(
                request_text,
                family,
                TEST_SIZE,
                TEST_WEIGHT,
                TEST_ITALIC,
                TEST_LETTER_SPACING,
                None,
            )
            .map_err(|error| format!("{error:?}"))?;
            Ok(started.elapsed())
        })
        .collect()
}

fn durations(values: &[Duration]) -> Vec<u128> {
    values.iter().map(Duration::as_nanos).collect()
}
