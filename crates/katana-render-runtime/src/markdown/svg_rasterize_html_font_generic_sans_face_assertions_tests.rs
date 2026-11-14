use super::{
    Source, assert_same_face, assert_same_face_bytes, font_path, inspect_source,
    single_face_database,
};
use std::sync::Arc;

const NOTO_SANS: &str = "assets/fonts/NotoSans-Regular.ttf";
const YELLOWTAIL: &str =
    "../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/Yellowtail-Regular.ttf";

#[test]
fn diagnostic_identity_distinguishes_real_font_source_kinds() -> Result<(), String> {
    let path = font_path(NOTO_SANS);
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    let sources = [
        Source::Binary(Arc::new(bytes.clone())),
        Source::File(path.clone()),
        Source::SharedFile(path.clone(), Arc::new(bytes.clone())),
    ];
    let expected = [
        "<binary>".to_owned(),
        path.display().to_string(),
        path.display().to_string(),
    ];
    let mut reference = None;
    for (source, expected_source) in sources.into_iter().zip(expected) {
        let report = inspect_source(source, &bytes)?;
        assert_eq!(report["source"].as_str(), Some(expected_source.as_str()));
        let identity = identity_fields(&report);
        if let Some(reference) = &reference {
            assert_eq!(&identity, reference);
        } else {
            reference = Some(identity);
        }
    }
    Ok(())
}

const FACE_IDENTITY_FIELDS: [&str; 3] = ["faceIndex", "postScriptName", "weight"];

fn identity_fields(report: &serde_json::Value) -> [serde_json::Value; FACE_IDENTITY_FIELDS.len()] {
    FACE_IDENTITY_FIELDS.map(|key| report[key].clone())
}

#[test]
fn diagnostic_face_bytes_reject_distinct_real_fonts() -> Result<(), String> {
    let noto_source = Source::File(font_path(NOTO_SANS));
    let (noto, noto_id) = single_face_database(noto_source.clone());
    let (same, same_id) = single_face_database(noto_source);
    let (yellowtail, yellowtail_id) = single_face_database(Source::File(font_path(YELLOWTAIL)));
    assert_same_face_bytes(&noto, noto_id, &same, same_id)?;
    assert_eq!(
        assert_same_face_bytes(&noto, noto_id, &yellowtail, yellowtail_id)
            .err()
            .as_deref(),
        Some("real face bytes or source indices differ")
    );
    Ok(())
}

#[test]
fn face_comparison_rejects_memory_backed_font_even_for_identical_bytes() -> Result<(), String> {
    let path = font_path(NOTO_SANS);
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    let (file, file_id) = single_face_database(Source::File(path));
    let (memory, memory_id) = single_face_database(Source::Binary(Arc::new(bytes)));
    assert_same_face_bytes(&file, file_id, &memory, memory_id)?;
    assert_eq!(
        assert_same_face(&memory, memory_id, &file, file_id)
            .err()
            .as_deref(),
        Some("known face is not file-backed")
    );
    assert_eq!(
        assert_same_face(&file, file_id, &memory, memory_id)
            .err()
            .as_deref(),
        Some("eager face is not file-backed")
    );
    Ok(())
}

#[test]
fn real_file_metrics_report_disappearance_and_recovery() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("NotoSans-Regular.ttf");
    let bytes = std::fs::read(font_path(NOTO_SANS)).map_err(|error| error.to_string())?;
    std::fs::write(&path, &bytes).map_err(|error| error.to_string())?;
    let (database, id) = single_face_database(Source::File(path.clone()));
    let expected = super::metrics(&database, id)?;
    std::fs::remove_file(&path).map_err(|error| error.to_string())?;
    assert_eq!(
        super::metrics(&database, id).err().as_deref(),
        Some("real face metrics missing")
    );
    std::fs::write(&path, &bytes).map_err(|error| error.to_string())?;
    assert_eq!(super::metrics(&database, id)?, expected);
    Ok(())
}
