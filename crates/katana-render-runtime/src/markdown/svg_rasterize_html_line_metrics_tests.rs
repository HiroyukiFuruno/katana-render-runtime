use super::{
    MAX_LINE_METRIC_CACHE, clear_html_line_metrics_cache, html_line_metrics,
    html_line_metrics_cache_len, html_text_baseline, html_text_baseline_for_text,
};

const FILE_METRIC_SIZE: f32 = 16.0;
const CORRUPTION_MASK: u8 = 0xff;

type FaceLineMetrics = Option<(f32, f32)>;
type ObservedFaceLineMetricPair = (FaceLineMetrics, FaceLineMetrics);

#[test]
fn rejected_face_metrics_never_read_the_legacy_font() -> Result<(), String> {
    use super::super::super::measurement_diagnostic::{self, FaceMetricScope};

    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES.to_vec());
    let id = database.faces().next().ok_or("fixture face missing")?.id;
    assert!(super::legacy_line_metrics(&database, id, FILE_METRIC_SIZE).is_some());
    let mut observation = measurement_diagnostic::enter(true);
    let detail = FaceMetricScope::new(true);
    for detail in [None, Some(&detail)] {
        let result =
            super::resolve_face_line_metrics(Err(()), &database, id, FILE_METRIC_SIZE, detail);
        assert_eq!(result, None);
    }
    let stats = observation.finish().face_inner;
    assert_eq!(stats.bridge_err, 1);
    assert_eq!(stats.legacy_calls, 0);
    Ok(())
}

#[test]
fn missing_face_source_returns_no_line_metrics() {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES.to_vec());
    let missing_face = resvg::usvg::fontdb::ID::dummy();

    assert_eq!(database.len(), 1);
    for detail in [false, true] {
        assert!(
            super::observed_line_metrics_for_face(
                &database,
                missing_face,
                FILE_METRIC_SIZE,
                detail
            )
            .is_none()
        );
    }
}

#[test]
fn malformed_file_is_rejected_by_legacy_line_metrics_and_recovers() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let bytes = super::super::font_file_fixture::FONT_BYTES;
    let fixture = super::super::font_file_fixture::create(directory.path().join("font.ttf"), bytes)
        .map_err(|error| error.to_string())?;
    let valid = line_metric_pair(&fixture)
        .0
        .ok_or("valid font metrics missing")?;
    assert_eq!(line_metric_pair(&fixture), (Some(valid), Some(valid)));
    let mut malformed = bytes.to_vec();
    malformed[0] ^= CORRUPTION_MASK;
    std::fs::write(&fixture.path, &malformed).map_err(|error| error.to_string())?;
    assert_malformed_file_rejected(&fixture, &malformed);
    std::fs::write(&fixture.path, bytes).map_err(|error| error.to_string())?;
    assert_eq!(line_metric_pair(&fixture), (Some(valid), Some(valid)));
    Ok(())
}

fn line_metric_pair(
    fixture: &super::super::font_file_fixture::FontFileFixture,
) -> ObservedFaceLineMetricPair {
    let read = |detail| {
        super::observed_line_metrics_for_face(
            &fixture.database,
            fixture.face_id,
            FILE_METRIC_SIZE,
            detail,
        )
    };
    (read(false), read(true))
}

fn assert_malformed_file_rejected(
    fixture: &super::super::font_file_fixture::FontFileFixture,
    malformed: &[u8],
) {
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

#[test]
fn system_font_baseline_uses_the_primary_font_strut() -> Result<(), String> {
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    for (size, height, baseline) in [
        (16.0, 24.0, 18.0),
        (17.0, 26.0, 19.0),
        (22.0, 33.0, 24.0),
        (30.0, 37.5, 30.0),
    ] {
        let actual = html_text_baseline(family, size, 700, false, height)
            .ok_or_else(|| "missing system font strut".to_string())?;
        assert_eq!(actual, baseline, "font size {size}");
    }
    Ok(())
}

#[test]
fn normal_and_explicit_baselines_use_their_css_strut_modes() -> Result<(), String> {
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    let footer = html_text_baseline_for_text(
        "この画面は架空の図書館サービスを用いた描画検証用fixtureです。",
        family,
        13.0,
        400,
        false,
        20.0,
        true,
    )
    .ok_or_else(|| "missing footer baseline".to_string())?;
    assert_eq!(footer, 14.0);
    let reversed =
        html_text_baseline_for_text("fixture日本語", family, 13.0, 400, false, 20.0, true)
            .ok_or_else(|| "missing reversed baseline".to_string())?;
    assert_eq!(reversed, 14.0);
    let badge = html_text_baseline_for_text("要件仕様 v1", family, 12.0, 700, false, 18.0, true)
        .ok_or_else(|| "missing badge baseline".to_string())?;
    assert_eq!(badge, 14.0);
    let explicit =
        html_text_baseline_for_text("fixture日本語", family, 13.0, 400, false, 20.0, false)
            .ok_or_else(|| "missing explicit baseline".to_string())?;
    assert_eq!(explicit, 15.0);
    Ok(())
}

#[test]
fn line_metrics_cache_clear_discards_previous_entries() -> Result<(), String> {
    clear_html_line_metrics_cache();
    assert_eq!(html_line_metrics_cache_len(), 0);
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    html_line_metrics("日本語", family, 13.0, 400, false)
        .ok_or_else(|| "missing line metrics".to_string())?;
    assert_eq!(html_line_metrics_cache_len(), 1);
    clear_html_line_metrics_cache();
    assert_eq!(html_line_metrics_cache_len(), 0);
    html_line_metrics("日本語", family, 13.0, 400, false)
        .ok_or_else(|| "missing line metrics after clear".to_string())?;
    assert_eq!(html_line_metrics_cache_len(), 1);
    Ok(())
}

#[test]
fn line_metrics_cache_keeps_its_limit_for_distinct_labels() -> Result<(), String> {
    clear_html_line_metrics_cache();
    let measure = |text: &str| {
        html_line_metrics(text, "Noto Sans, sans-serif", FILE_METRIC_SIZE, 400, false)
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

#[test]
fn japanese_system_font_normal_line_height_matches_chrome() -> Result<(), String> {
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    for (size, height) in [(16.0, 24.0), (17.0, 26.0), (22.0, 33.0)] {
        let (ascent, descent) = html_line_metrics("日本語", family, size, 700, false)
            .ok_or_else(|| "missing installed Japanese font metrics".to_string())?;
        assert_eq!(ascent + descent, height, "font size {size}");
    }
    let (ascent, descent) = html_line_metrics("日本語", family, 16.0, 400, false)
        .ok_or_else(|| "missing normal Japanese font metrics".to_string())?;
    assert_eq!(ascent + descent, 24.0);
    Ok(())
}

#[test]
fn japanese_system_font_advance_matches_chrome_dom() -> Result<(), String> {
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    let text = "検索結果は通信環境や蔵書の更新状況によって表示順が変わる場合があり";
    let width = super::super::shaped_text_width(text, family, 16.0, 400, false, 0.0, None)
        .ok_or_else(|| "missing shaped font advance".to_string())?;
    assert!((width - 527.531_25).abs() <= 1.0 / 64.0, "width {width}");
    Ok(())
}

#[test]
fn variable_system_font_axes_match_chrome_dom() -> Result<(), String> {
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    for (text, weight, expected) in [
        ("AV", 400, 26.625),
        ("AV", 700, 28.828_125),
        ("目的", 700, 44.0),
        ("1. ", 700, 20.406_25),
        ("1. 目的", 700, 64.406_25),
    ] {
        let actual = super::super::shaped_text_width(text, family, 22.0, weight, false, 0.0, None)
            .ok_or_else(|| "missing shaped system font".to_string())?;
        assert!(
            (actual - expected).abs() <= 1.0 / 64.0,
            "{text} {weight}: {actual}"
        );
    }
    Ok(())
}
