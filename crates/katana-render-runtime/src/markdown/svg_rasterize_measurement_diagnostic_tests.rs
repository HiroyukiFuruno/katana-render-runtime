use super::super::SvgRasterizeOps;
use super::{
    Stats, baseline_finish, baseline_start, enter, line_finish, line_metrics_detail, line_start,
};

const TEST_FONT_FAMILY: &str = "Noto Sans";
const TEST_FONT_SIZE: f32 = 12.0;
const TEST_FONT_WEIGHT: u16 = 400;
const TEST_LINE_HEIGHT: f32 = 18.0;

#[test]
fn real_font_metrics_record_cache_hits_and_misses() {
    SvgRasterizeOps::clear_html_line_metrics_cache();
    let mut scope = enter(true);
    let metrics = real_line_metrics();
    assert!(metrics.is_some());
    assert_eq!(real_line_metrics(), metrics);
    let baseline = real_primary_baseline();
    assert!(baseline.is_some());
    assert_eq!(real_primary_baseline(), baseline);
    let stats = scope.finish();
    assert_eq!(
        (stats.line_calls, stats.line_hits, stats.line_misses),
        (2, 1, 1)
    );
    assert_eq!(
        (
            stats.baseline_calls,
            stats.baseline_hits,
            stats.baseline_misses
        ),
        (2, 1, 1)
    );
    SvgRasterizeOps::clear_html_line_metrics_cache();
}

fn real_primary_baseline() -> Option<f32> {
    SvgRasterizeOps::html_text_baseline_for_text(
        "AA",
        TEST_FONT_FAMILY,
        TEST_FONT_SIZE,
        TEST_FONT_WEIGHT,
        false,
        TEST_LINE_HEIGHT,
        false,
    )
}

fn real_line_metrics() -> Option<(f32, f32)> {
    SvgRasterizeOps::html_line_metrics(
        "AA",
        TEST_FONT_FAMILY,
        TEST_FONT_SIZE,
        TEST_FONT_WEIGHT,
        false,
    )
}

#[test]
fn disabled_scope_does_not_start_clock_or_record() {
    let mut scope = enter(false);
    assert!(line_metrics_detail(false).is_none());
    assert!(line_start().is_none());
    assert!(baseline_start().is_none());
    line_finish(false, None);
    baseline_finish(false, None);
    assert_eq!(scope.finish(), Stats::default());
}

#[test]
fn line_detail_drop_records_faces_and_closes_early_return_phase() {
    let mut scope = enter(true);
    let early_return = || {
        let mut detail = line_metrics_detail(true)?;
        detail.start_matching();
        detail.record_face_source(&resvg::usvg::fontdb::Source::File("font.ttf".into()));
        None::<()>
    };
    assert_eq!(early_return(), None);
    let stats = scope.finish();
    assert_eq!(stats.line_metric_face_file_calls, 1);
    assert_eq!(stats.line_metric_face_binary_calls, 0);
    assert_eq!(stats.line_metric_face_shared_calls, 0);
}

#[test]
fn shared_real_font_detail_is_recorded_and_late_drop_does_not_leak() -> Result<(), String> {
    use resvg::usvg::fontdb::{Database, Source};
    use std::{path::PathBuf, sync::Arc};

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    let source = Source::SharedFile(path, Arc::new(bytes));
    let mut database = Database::new();
    assert!(!database.load_font_source(source.clone()).is_empty());
    let mut scope = enter(true);
    let mut detail = line_metrics_detail(true).ok_or("enabled detail scope")?;
    detail.record_face_source(&source);
    drop(detail);
    assert_eq!(scope.finish().line_metric_face_shared_calls, 1);
    let mut scope = enter(true);
    let detail = line_metrics_detail(true).ok_or("enabled detail scope")?;
    assert_eq!(scope.finish(), Stats::default());
    drop(detail);
    let mut next = enter(true);
    assert_eq!(next.finish(), Stats::default());
    Ok(())
}

#[test]
fn enabled_scope_records_misses_without_a_clock() {
    let mut scope = enter(true);
    assert!(line_start().is_some());
    assert!(baseline_start().is_some());
    line_finish(false, None);
    baseline_finish(false, None);
    assert_eq!(
        scope.finish(),
        Stats {
            line_calls: 1,
            line_misses: 1,
            baseline_calls: 1,
            baseline_misses: 1,
            ..Stats::default()
        }
    );
}

#[test]
fn disabled_nested_scope_restores_enabled_outer() -> Result<(), &'static str> {
    let mut outer = enter(true);
    line_finish(true, None);
    let mut detail = line_metrics_detail(true).ok_or("enabled detail scope")?;
    detail.record_face_source(&resvg::usvg::fontdb::Source::Binary(std::sync::Arc::new(
        Vec::<u8>::new(),
    )));
    drop(detail);
    let mut disabled = enter(false);
    assert!(line_metrics_detail(false).is_none());
    line_finish(false, None);
    assert_eq!(disabled.finish(), Stats::default());
    baseline_finish(false, None);
    assert_eq!(
        outer.finish(),
        Stats {
            line_calls: 1,
            line_hits: 1,
            baseline_calls: 1,
            baseline_misses: 1,
            line_metric_face_binary_calls: 1,
            ..Stats::default()
        }
    );
    Ok(())
}

#[test]
fn unwind_and_double_finish_restore_outer_lifo_state() {
    let mut outer = enter(true);
    line_finish(true, None);
    let result = std::panic::catch_unwind(|| {
        let _inner = enter(true);
        baseline_finish(true, None);
        std::panic::resume_unwind(Box::new("diagnostic unwind"));
    });
    assert!(result.is_err());
    assert!(line_start().is_some());
    baseline_finish(false, None);
    assert_eq!(
        outer.finish(),
        Stats {
            line_calls: 1,
            line_hits: 1,
            baseline_calls: 1,
            baseline_misses: 1,
            ..Stats::default()
        }
    );
    assert_eq!(outer.finish(), Stats::default());
}

#[test]
fn inner_double_finish_does_not_consume_outer_scope() {
    let mut outer = enter(true);
    line_finish(true, None);
    let mut inner = enter(true);
    line_finish(false, None);
    let inner_stats = inner.finish();
    assert_eq!(inner_stats.line_misses, 1);
    assert_eq!(inner.finish(), Stats::default());
    line_finish(true, None);
    assert_eq!(outer.finish().line_calls, 2);
}
