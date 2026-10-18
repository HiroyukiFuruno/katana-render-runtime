use super::super::super::super::measurement_diagnostic::{self, Stats};
use super::super::super::with_render_scoped_stamp_batch;
use super::super::{clear_html_line_metrics_cache, html_line_metrics};
use super::{MAX_PERSISTENT_METRICS, Request, VALUES, lookup};

type Metrics = (f32, f32);

const TEXT: &str = "AVoffice";
const FAMILY: &str = "Noto Sans";
const SIZE: f32 = 18.25;
const BOLD_FONT_WEIGHT: u16 = 700;
const EVICTION_REQUEST_COUNT: usize = 9;
const WEIGHT: u16 = 400;

fn clear_persistent() {
    VALUES.with(|cache| cache.borrow_mut().clear());
    clear_html_line_metrics_cache();
}

fn real_metrics() -> Option<(f32, f32)> {
    html_line_metrics(TEXT, FAMILY, SIZE, WEIGHT, false)
}

fn first_metrics() -> Result<(Metrics, bool), String> {
    clear_html_line_metrics_cache();
    let metrics = real_metrics().ok_or("first real metrics missing")?;
    Ok((
        metrics,
        Request::new(TEXT, FAMILY, SIZE, WEIGHT, false).is_some(),
    ))
}

fn observed_metrics() -> Result<(Metrics, Stats), String> {
    clear_html_line_metrics_cache();
    let mut diagnostic = measurement_diagnostic::enter(true);
    let metrics = real_metrics().ok_or("observed real metrics missing")?;
    Ok((metrics, diagnostic.finish()))
}

fn assert_repaint(stats: &Stats, reusable: bool) {
    let expected = if reusable { (1, 0) } else { (0, 1) };
    assert_eq!((stats.line_hits, stats.line_misses), expected);
    if reusable {
        assert_eq!(
            stats.line_metric_face_file_calls
                + stats.line_metric_face_binary_calls
                + stats.line_metric_face_shared_calls,
            0
        );
    }
}

#[test]
fn real_line_metrics_survive_frame_clear_only_with_valid_factory_token() -> Result<(), String> {
    clear_persistent();
    let (first, reusable) = with_render_scoped_stamp_batch(first_metrics)?;
    #[cfg(target_os = "macos")]
    assert!(
        reusable,
        "macOS regression must exercise a real persistent hit"
    );
    let (second, stats) = with_render_scoped_stamp_batch(observed_metrics)?;
    assert_eq!(
        (first.0.to_bits(), first.1.to_bits()),
        (second.0.to_bits(), second.1.to_bits())
    );
    assert_repaint(&stats, reusable);
    let (outside, stats) = observed_metrics()?;
    assert_eq!(outside, first);
    assert_repaint(&stats, false);
    clear_persistent();
    Ok(())
}

fn reject_distinct_inputs() {
    assert!(Request::new("", FAMILY, SIZE, WEIGHT, false).is_none());
    let count = VALUES.with(|cache| cache.borrow().len());
    assert!(html_line_metrics("", FAMILY, SIZE, WEIGHT, false).is_none());
    assert_eq!(VALUES.with(|cache| cache.borrow().len()), count);
    let variants = [
        ("different text", FAMILY, SIZE, WEIGHT, false),
        (TEXT, "serif", SIZE, WEIGHT, false),
        (TEXT, FAMILY, SIZE + 1.0, WEIGHT, false),
        (TEXT, FAMILY, SIZE, BOLD_FONT_WEIGHT, false),
        (TEXT, FAMILY, SIZE, WEIGHT, true),
    ];
    for (text, family, size, weight, italic) in variants {
        if let Some(request) = Request::new(text, family, size, weight, italic) {
            assert!(lookup(&request).is_none());
        }
    }
}

#[test]
fn distinct_inputs_and_nested_unvalidated_scope_cannot_reuse_metrics() -> Result<(), String> {
    clear_persistent();
    with_render_scoped_stamp_batch(|| {
        real_metrics().ok_or("first real metrics missing")?;
        reject_distinct_inputs();
        with_render_scoped_stamp_batch(|| {
            assert!(Request::new(TEXT, FAMILY, SIZE, WEIGHT, false).is_none());
            let (_, stats) = observed_metrics()?;
            assert_repaint(&stats, false);
            Ok::<_, String>(())
        })
    })?;
    clear_persistent();
    Ok(())
}

#[test]
fn persistent_metrics_are_bounded_and_factory_is_required_outside_frame() -> Result<(), String> {
    clear_persistent();
    with_render_scoped_stamp_batch(|| {
        for index in 0..MAX_PERSISTENT_METRICS + 32 {
            let text = format!("AVoffice-{index}");
            assert!(html_line_metrics(&text, FAMILY, SIZE, WEIGHT, false).is_some());
            assert!(VALUES.with(|cache| cache.borrow().len()) <= MAX_PERSISTENT_METRICS);
        }
        Ok::<_, String>(())
    })?;
    #[cfg(target_os = "macos")]
    assert_eq!(
        VALUES.with(|cache| cache.borrow().len()),
        MAX_PERSISTENT_METRICS
    );
    assert!(Request::new(TEXT, FAMILY, SIZE, WEIGHT, false).is_none());
    clear_persistent();
    Ok(())
}

#[cfg(unix)]
fn evict_original_factory_proof() {
    for index in 0..EVICTION_REQUEST_COUNT {
        let family = format!("Noto Sans, serif, missing-{index}");
        with_render_scoped_stamp_batch(|| {
            assert!(Request::new(TEXT, &family, SIZE, WEIGHT, false).is_some());
        });
    }
}

#[cfg(unix)]
#[test]
fn replaced_factory_proof_cannot_hit_old_persistent_metrics() -> Result<(), String> {
    clear_persistent();
    let (first, reusable) = with_render_scoped_stamp_batch(first_metrics)?;
    assert!(reusable);
    evict_original_factory_proof();
    let (replacement, stats) = with_render_scoped_stamp_batch(observed_metrics)?;
    assert_eq!(replacement, first);
    assert_repaint(&stats, false);
    clear_persistent();
    Ok(())
}

#[cfg(unix)]
#[path = "svg_rasterize_html_line_metrics_persistent_retry_tests.rs"]
mod retry_tests;

#[cfg(unix)]
#[path = "svg_rasterize_html_line_metrics_persistent_boundary_tests.rs"]
mod boundary_tests;
