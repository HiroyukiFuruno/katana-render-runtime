use super::super::{ACTIVE_SCOPE, commit_file_font};
use super::*;
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use resvg::usvg::fontdb::ID;

#[path = "svg_rasterize_text_harfrust_face_cache_frame_budget_tests.rs"]
mod budget_tests;
#[path = "svg_rasterize_text_harfrust_face_cache_frame_generation_tests.rs"]
mod generation_tests;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

fn inner_font(database: &Database, face: ID) -> Result<FontHandle, String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let font = font_for_face(database, face).ok_or("inner real font missing")?;
        shape_and_commit(database, face, &font, "inner frame")?;
        Ok(font)
    })
}

fn assert_retained_owner(depth: usize, font: &FontHandle) {
    ACTIVE_SCOPE.with(|cache| {
        let cache = cache.borrow();
        assert_eq!(cache.depth, depth);
        let entries = cache.entry.iter().chain(cache.spare.iter());
        assert!(entries.clone().any(|entry| {
            entry.font.retained_source_bytes > 0
                && std::ptr::eq(entry.font.metrics(), font.metrics())
        }));
        assert!(
            entries
                .map(|entry| entry.font.retained_source_bytes)
                .sum::<usize>()
                <= MAX_SOURCE_BYTES
        );
    });
}

fn assert_released() {
    ACTIVE_SCOPE.with(|cache| {
        let cache = cache.borrow();
        assert_eq!(cache.depth, 0);
        assert!(cache.entry.is_none() && cache.spare.is_none());
    });
}

#[test]
fn frame_scope_retains_real_metrics_across_inners_and_next_frame_is_cold() -> Result<(), String> {
    let (database, face) = binary_database()?;
    let previous = SvgRasterizeOps::with_html_font_face_cache_scope(|| {
        retained_metrics_between_inners(&database, face)
    })?;
    assert_released();
    SvgRasterizeOps::with_html_font_face_cache_scope(|| {
        assert_cold_next_frame(&database, face, &previous)
    })?;
    assert_released();
    Ok(())
}

fn retained_metrics_between_inners(database: &Database, face: ID) -> Result<FontHandle, String> {
    let first = inner_font(database, face)?;
    assert_retained_owner(1, &first);
    let next = inner_font(database, face)?;
    assert!(std::ptr::eq(first.metrics(), next.metrics()));
    assert_eq!(first.metrics(), next.metrics());
    assert_eq!(
        shape_width(&first, 18.25, "AVoffice")?,
        shape_width(&next, 18.25, "AVoffice")?
    );
    drop(next);
    assert_retained_owner(1, &first);
    Ok(first)
}

fn assert_cold_next_frame(
    database: &Database,
    face: ID,
    previous: &FontHandle,
) -> Result<(), String> {
    assert_all_handles_cleared();
    let fresh = inner_font(database, face)?;
    /* WHY: 前frameの比較用cloneを保持し、allocatorのaddress再利用とcold loadを区別する。 */
    assert!(!std::ptr::eq(previous.metrics(), fresh.metrics()));
    assert_eq!(previous.metrics(), fresh.metrics());
    assert_retained_owner(1, &fresh);
    Ok(())
}

#[test]
fn populated_frame_scope_releases_on_error_unwind_and_restores_external_depth() -> Result<(), String>
{
    let (database, face) = binary_database()?;
    check_populated_error(&database, face)?;
    check_populated_unwind(&database, face)?;
    check_external_depth_restore(&database, face)
}

fn check_populated_error(database: &Database, face: ID) -> Result<(), String> {
    let failed = SvgRasterizeOps::with_html_font_face_cache_scope(|| -> Result<(), String> {
        let font = inner_font(database, face)?;
        assert_retained_owner(1, &font);
        Err("loaded frame failure".to_owned())
    });
    assert_eq!(failed, Err("loaded frame failure".to_owned()));
    assert_released();
    Ok(())
}

fn check_populated_unwind(database: &Database, face: ID) -> Result<(), String> {
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        SvgRasterizeOps::with_html_font_face_cache_scope(|| -> Result<(), String> {
            let font = inner_font(database, face)?;
            assert_retained_owner(1, &font);
            std::panic::resume_unwind(Box::new("loaded frame unwind"))
        })
    }));
    match unwound {
        Err(payload) => assert_eq!(payload.downcast_ref::<&str>(), Some(&"loaded frame unwind")),
        Ok(result) => return Err(format!("loaded frame did not unwind: {result:?}")),
    }
    assert_released();
    Ok(())
}

fn check_external_depth_restore(database: &Database, face: ID) -> Result<(), String> {
    let external = FontFaceCacheScope::enter();
    let previous = inner_font(database, face)?;
    assert_retained_owner(1, &previous);
    SvgRasterizeOps::with_html_font_face_cache_scope(|| -> Result<(), String> {
        let nested = inner_font(database, face)?;
        assert!(std::ptr::eq(previous.metrics(), nested.metrics()));
        assert_retained_owner(2, &nested);
        Ok(())
    })?;
    assert_retained_owner(1, &previous);
    drop(external);
    assert_released();
    Ok(())
}
