use super::super::font_file_fixture::{self, FONT_BYTES, FontFileFixture};
use super::{
    MAX_LINE_METRIC_CACHE, clear_html_line_metrics_cache, html_line_metrics,
    html_line_metrics_cache_len,
};
use resvg::usvg::fontdb::{Database, ID};

const CORRUPTION_MASK: u8 = 0xff;
const FONT_SIZE: f32 = 16.0;

type FaceLineMetrics = Option<(f32, f32)>;
type ObservedFaceLineMetricPair = (FaceLineMetrics, FaceLineMetrics);

#[test]
fn missing_face_source_returns_no_line_metrics() {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let missing_face = ID::dummy();

    assert_eq!(database.len(), 1);
    for detail in [false, true] {
        assert!(
            super::observed_line_metrics_for_face(&database, missing_face, FONT_SIZE, detail)
                .is_none()
        );
    }
}

#[test]
fn malformed_file_is_rejected_by_legacy_line_metrics_and_recovers() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = font_file_fixture::create(directory.path().join("font.ttf"), FONT_BYTES)
        .map_err(|error| error.to_string())?;
    let valid = line_metric_pair(&fixture)
        .0
        .ok_or("valid font metrics missing")?;
    assert_eq!(line_metric_pair(&fixture), (Some(valid), Some(valid)));
    let mut malformed = FONT_BYTES.to_vec();
    malformed[0] ^= CORRUPTION_MASK;
    std::fs::write(&fixture.path, &malformed).map_err(|error| error.to_string())?;
    assert_malformed_file_rejected(&fixture, &malformed);
    std::fs::write(&fixture.path, FONT_BYTES).map_err(|error| error.to_string())?;
    assert_eq!(line_metric_pair(&fixture), (Some(valid), Some(valid)));
    Ok(())
}

#[test]
fn line_metrics_cache_keeps_its_limit_for_distinct_labels() -> Result<(), String> {
    clear_html_line_metrics_cache();
    let measure = |text: &str| {
        html_line_metrics(text, "Noto Sans, sans-serif", FONT_SIZE, 400, false)
            .ok_or_else(|| format!("missing line metrics for {text}"))
    };
    let first = "catalog item";
    let expected = measure(first)?;
    for index in 0..MAX_LINE_METRIC_CACHE {
        measure(&format!("catalog item {index}"))?;
        assert_eq!(
            html_line_metrics_cache_len(),
            (index + 2).min(MAX_LINE_METRIC_CACHE)
        );
    }
    assert_eq!(measure(first)?, expected);
    assert_eq!(html_line_metrics_cache_len(), MAX_LINE_METRIC_CACHE);
    clear_html_line_metrics_cache();
    Ok(())
}

fn line_metric_pair(fixture: &FontFileFixture) -> ObservedFaceLineMetricPair {
    let read = |detail| {
        super::observed_line_metrics_for_face(&fixture.database, fixture.face_id, FONT_SIZE, detail)
    };
    (read(false), read(true))
}

fn assert_malformed_file_rejected(fixture: &FontFileFixture, malformed: &[u8]) {
    let parsed = fixture
        .database
        .with_face_data(fixture.face_id, |data, index| {
            assert_eq!(data, malformed);
            rustybuzz::ttf_parser::Face::parse(data, index).is_ok()
        });
    assert_eq!(parsed, Some(false));
    let mut scope = super::super::super::measurement_diagnostic::enter(true);
    assert_eq!(line_metric_pair(fixture), (None, None));
    let stats = scope.finish();
    assert_eq!(stats.face_inner.bridge_none, 1);
    assert_eq!(stats.face_inner.legacy_calls, 1);
}
